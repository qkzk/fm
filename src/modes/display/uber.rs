use anyhow::{anyhow, bail, Context, Result};

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::common::{
    filename_from_path, hash_path, path_to_string, FFMPEG, FONTIMAGE, LIBREOFFICE, PDFINFO,
    PDFTOPPM, RSVG_CONVERT, THUMBNAIL_PATH_NO_EXT, THUMBNAIL_PATH_PNG, TMP_THUMBNAILS_DIR,
};
use crate::io::{execute_and_capture_output, execute_and_output_no_log};
use crate::log_info;
use crate::modes::ExtensionKind;

/// Different kind of ueberzug previews.
/// it's used to know which program should be run to build the images.
/// pdfs, or office documents can't be displayed directly in the terminal and require
/// to be converted first.
#[derive(Default)]
pub enum Kind {
    Font,
    Image,
    Office,
    Pdf,
    Svg,
    Video,
    #[default]
    Unknown,
}

impl Kind {
    fn allow_multiples(&self) -> bool {
        matches!(self, Self::Pdf)
    }

    pub fn for_first_line(&self) -> &str {
        match self {
            Self::Font => "a font",
            Self::Image => "an image",
            Self::Office => "an office document",
            Self::Pdf => "a pdf",
            Self::Svg => "an svg image",
            Self::Video => "a video",
            Self::Unknown => "Unknown",
        }
    }
}

impl From<ExtensionKind> for Kind {
    fn from(kind: ExtensionKind) -> Self {
        match &kind {
            ExtensionKind::Font => Self::Font,
            ExtensionKind::Image => Self::Image,
            ExtensionKind::Office => Self::Office,
            ExtensionKind::Pdf => Self::Pdf,
            ExtensionKind::Svg => Self::Svg,
            ExtensionKind::Video => Self::Video,
            _ => Self::Unknown,
        }
    }
}
impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Font => write!(f, "font"),
            Self::Image => write!(f, "image"),
            Self::Office => write!(f, "office"),
            Self::Pdf => write!(f, "pdf"),
            Self::Svg => write!(f, "svg"),
            Self::Unknown => write!(f, "unknown"),
            Self::Video => write!(f, "video"),
        }
    }
}

pub fn path_is_video<P: AsRef<Path>>(path: P) -> bool {
    let Some(ext) = path.as_ref().extension() else {
        return false;
    };
    matches!(
        ext.to_string_lossy().as_ref(),
        "mkv" | "webm" | "mpeg" | "mp4" | "avi" | "flv" | "mpg" | "wmv" | "m4v" | "mov"
    )
}

/// Holds an instance of [`ueberzug::Ueberzug`] and a few information about the display.
/// it's used to display the image itself, calling `draw` with parameters for its position and dimension.
pub struct Ueber {
    since: Instant,
    pub kind: Kind,
    pub identifier: String,
    images: Vec<PathBuf>,
    length: usize,
    pub index: usize,
    ueberzug: ueberzug::Ueberzug,
}

impl Ueber {
    fn new(kind: Kind, identifier: String, images: Vec<PathBuf>) -> Self {
        let ueberzug = ueberzug::Ueberzug::new();
        let index = 0;
        let length = images.len();
        let since = Instant::now();
        Self {
            since,
            kind,
            identifier,
            images,
            length,
            index,
            ueberzug,
        }
    }
    /// Only affect pdf thumbnail. Will decrease the index if possible.
    pub fn up_one_row(&mut self) {
        if self.kind.allow_multiples() && self.index > 0 {
            self.index -= 1;
        }
    }

    /// Only affect pdf thumbnail. Will increase the index if possible.
    pub fn down_one_row(&mut self) {
        if self.kind.allow_multiples() && self.index + 1 < self.len() {
            self.index += 1;
        }
    }

    /// 0 for every kind except pdf where it's the number of pages.
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn video_index(&self) -> usize {
        let elapsed = self.since.elapsed().as_secs() as usize;
        elapsed % self.images.len()
    }

    fn image_index(&self) -> usize {
        if matches!(self.kind, Kind::Video) {
            self.video_index()
        } else {
            self.index
        }
    }

    /// Draw the image with ueberzug in the current window.
    /// The position is absolute, which is problematic when the app is embeded into a floating terminal.
    /// The whole struct instance is dropped when the preview is reset and the image is deleted.
    pub fn draw(&self, x: u16, y: u16, width: u16, height: u16) {
        log_info!(
            "ueber draws {image} {index}",
            image = self.images[self.index].display(),
            index = self.index
        );
        self.ueberzug.draw(&ueberzug::UeConf {
            identifier: &self.identifier,
            path: &self.images[self.image_index()].to_string_lossy(),
            x,
            y,
            width: Some(width),
            height: Some(height),
            scaler: Some(ueberzug::Scalers::FitContain),
            ..Default::default()
        });
    }
}

/// Build an [`Ueber`] instance for a given source.
/// All thumbnails are built here.
pub struct UeberBuilder {
    kind: Kind,
    source: PathBuf,
}

impl UeberBuilder {
    pub fn video_thumbnails(hashed_path: &str) -> [String; 4] {
        [
            format!("{TMP_THUMBNAILS_DIR}/{hashed_path}_1.jpg"),
            format!("{TMP_THUMBNAILS_DIR}/{hashed_path}_2.jpg"),
            format!("{TMP_THUMBNAILS_DIR}/{hashed_path}_3.jpg"),
            format!("{TMP_THUMBNAILS_DIR}/{hashed_path}_4.jpg"),
        ]
    }

    pub fn new(source: &Path, kind: Kind) -> Self {
        let source = source.to_path_buf();
        Self { source, kind }
    }

    pub fn build(self) -> Result<Ueber> {
        match &self.kind {
            Kind::Font => self.build_font(),
            Kind::Image => self.build_image(),
            Kind::Office => self.build_office(),
            Kind::Pdf => self.build_pdf(),
            Kind::Svg => self.build_svg(),
            Kind::Video => self.build_video(),
            _ => Err(anyhow!("Unknown kind {kind}", kind = self.kind)),
        }
    }

    fn build_office(self) -> Result<Ueber> {
        let calc_str = path_to_string(&self.source);
        Self::convert_office_to_pdf(&calc_str)?;
        let pdf = Self::office_to_pdf_filename(
            self.source
                .file_name()
                .context("couldn't extract filename")?,
        )?;
        if !pdf.exists() {
            bail!("couldn't convert {calc_str} to pdf");
        }
        let identifier = filename_from_path(&pdf)?.to_owned();
        Thumbnail::create(&self.kind, pdf.to_string_lossy().as_ref());
        let images = Self::make_pdf_images_paths(Self::get_pdf_length(&pdf)?)?;
        std::fs::remove_file(&pdf)?;

        Ok(Ueber::new(Kind::Pdf, identifier, images))
    }

    fn convert_office_to_pdf(calc_str: &str) -> Result<std::process::Output> {
        let args = ["--convert-to", "pdf", "--outdir", "/tmp", calc_str];
        execute_and_output_no_log(LIBREOFFICE, args)
    }

    fn office_to_pdf_filename(filename: &OsStr) -> Result<PathBuf> {
        let mut pdf_path = PathBuf::from("/tmp");
        pdf_path.push(filename);
        pdf_path.set_extension("pdf");
        Ok(pdf_path)
    }

    fn make_pdf_images_paths(length: usize) -> Result<Vec<PathBuf>> {
        let images = (1..length + 1)
            .map(|index| PathBuf::from(format!("{THUMBNAIL_PATH_NO_EXT}-{index}.jpg")))
            .filter(|p| p.exists())
            .collect();
        Ok(images)
    }

    fn get_pdf_length(path: &Path) -> Result<usize> {
        let output =
            execute_and_capture_output(PDFINFO, &[path.to_string_lossy().to_string().as_ref()])?;
        let line = output.lines().find(|line| line.starts_with("Pages: "));

        match line {
            Some(line) => {
                let page_count_str = line.split_whitespace().nth(1).unwrap();
                let page_count = page_count_str.parse::<usize>()?;
                log_info!(
                    "pdf {path} has {page_count_str} pages",
                    path = path.display()
                );
                Ok(page_count)
            }
            None => Err(anyhow!("Couldn't find the page number")),
        }
    }

    fn build_pdf(self) -> Result<Ueber> {
        let length = Self::get_pdf_length(&self.source)?;
        let identifier = filename_from_path(&self.source)?.to_owned();
        Thumbnail::create(&self.kind, self.source.to_string_lossy().as_ref());
        let images = Self::make_pdf_images_paths(length)?;
        log_info!("build_pdf images: {images:?}");
        Ok(Ueber::new(self.kind, identifier, images))
    }

    fn build_video(self) -> Result<Ueber> {
        let path_str = self
            .source
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Thumbnail::create(&self.kind, path_str);
        let hashed_path = hash_path(path_str);
        let images: Vec<PathBuf> = Self::video_thumbnails(&hashed_path)
            .map(PathBuf::from)
            .into_iter()
            .filter(|p| p.exists())
            .collect();
        let identifier = filename_from_path(&self.source)?.to_owned();
        Ok(Ueber::new(self.kind, identifier, images))
    }

    fn build_single_image(self, images: Vec<PathBuf>) -> Result<Ueber> {
        let identifier = filename_from_path(&self.source)?.to_owned();
        Ok(Ueber::new(self.kind, identifier, images))
    }

    fn build_font(self) -> Result<Ueber> {
        let path_str = self
            .source
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Thumbnail::create(&self.kind, path_str);
        let p = PathBuf::from(THUMBNAIL_PATH_PNG);
        let images = if p.exists() { vec![p] } else { vec![] };
        self.build_single_image(images)
    }

    fn build_image(self) -> Result<Ueber> {
        let images = vec![self.source.clone()];
        self.build_single_image(images)
    }

    fn build_svg(self) -> Result<Ueber> {
        let path_str = self
            .source
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Thumbnail::create(&self.kind, path_str);
        let p = PathBuf::from(THUMBNAIL_PATH_PNG);
        let images = if p.exists() { vec![p] } else { vec![] };
        self.build_single_image(images)
    }
}

pub struct Thumbnail;

impl Thumbnail {
    fn create(kind: &Kind, path_str: &str) {
        let _ = match kind {
            Kind::Font => Self::create_font(path_str),
            Kind::Office => Self::create_office(path_str),
            Kind::Pdf => Self::create_pdf(path_str),
            Kind::Svg => Self::create_svg(path_str),
            Kind::Video => Self::create_video(path_str),

            _ => Ok(()),
        };
    }

    fn create_font(path_str: &str) -> Result<()> {
        Self::execute(FONTIMAGE, &["-o", THUMBNAIL_PATH_PNG, path_str])
    }

    fn create_office(path_str: &str) -> Result<()> {
        Self::create_pdf(path_str)
    }

    fn create_svg(path_str: &str) -> Result<()> {
        Thumbnail::execute(
            RSVG_CONVERT,
            &["--keep-aspect-ratio", path_str, "-o", THUMBNAIL_PATH_PNG],
        )
    }

    pub fn create_video(path_str: &str) -> Result<()> {
        let rand = hash_path(path_str);
        let ffmpeg_filename = format!("{TMP_THUMBNAILS_DIR}/{rand}_%d.jpg",);
        let images_paths = UeberBuilder::video_thumbnails(&rand);
        if Path::new(&images_paths[0]).exists() && !is_older_than_a_week(&images_paths[0]) {
            return Ok(());
        }
        for image in &images_paths {
            let _ = std::fs::remove_file(image);
        }

        let ffmpeg_args = [
            "-i",
            path_str,
            "-an",
            "-sn",
            "-vf",
            "fps=1/100,scale=320:-1",
            "-threads",
            "2",
            "-frames:v",
            "4",
            &ffmpeg_filename,
            // &format!("{THUMBNAIL_PATH_NO_EXT}_%d.jpg"),
        ];
        Thumbnail::execute(FFMPEG, &ffmpeg_args)
    }

    fn create_pdf(path_str: &str) -> Result<()> {
        Self::execute(
            PDFTOPPM,
            &[
                "-jpeg",
                "-jpegopt",
                "quality=75",
                path_str,
                THUMBNAIL_PATH_NO_EXT,
            ],
        )
    }

    fn execute(exe: &str, args: &[&str]) -> Result<()> {
        let output = execute_and_output_no_log(exe, args.to_owned())?;
        // if !output.stderr.is_empty() {
        log_info!(
            "make thumbnail output: {} {}",
            String::from_utf8(output.stdout).unwrap_or_default(),
            String::from_utf8(output.stderr).unwrap_or_default()
        );
        // }
        Ok(())
    }
}

const ONE_WEEK_IN_SECONDS: u64 = 7 * 24 * 60 * 60;

fn is_older_than_a_week(path: &str) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return true;
    };
    let Ok(creation) = metadata.created() else {
        return true;
    };
    let current_time = SystemTime::now();
    let Ok(duration) = current_time.duration_since(creation) else {
        return true;
    };
    duration > Duration::from_secs(ONE_WEEK_IN_SECONDS)
}
