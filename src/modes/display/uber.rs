use anyhow::{anyhow, bail, Context, Result};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use crate::common::{
    filename_from_path, path_to_string, FFMPEG, FONTIMAGE, LIBREOFFICE, PDFINFO, PDFTOPPM,
    RSVG_CONVERT, THUMBNAIL_PATH_NO_EXT, THUMBNAIL_PATH_PNG, THUMBNAIL_PDF_PATH,
};
use crate::io::{execute_and_capture_output, execute_and_output_no_log};
use crate::log_info;

use super::ExtensionKind;

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
}

impl From<ExtensionKind> for Kind {
    fn from(val: ExtensionKind) -> Self {
        match &val {
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

pub struct Ueber {
    since: Instant,
    kind: Kind,
    identifier: String,
    images: Vec<PathBuf>,
    length: usize,
    pub index: usize,
    ueberzug: ueberzug::Ueberzug,
}

impl Ueber {
    fn new(kind: Kind, identifier: String, images: Vec<PathBuf>, length: usize) -> Self {
        let ueberzug = ueberzug::Ueberzug::new();
        let index = 0;
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

pub struct UeberBuilder {
    kind: Kind,
    source: PathBuf,
}

impl UeberBuilder {
    // TODO! don't use static str but the str defined in constants
    const VIDEO_THUMBNAILS: [&'static str; 6] = [
        "/tmp/fm_thumbnail_1.jpg",
        "/tmp/fm_thumbnail_2.jpg",
        "/tmp/fm_thumbnail_3.jpg",
        "/tmp/fm_thumbnail_4.jpg",
        "/tmp/fm_thumbnail_5.jpg",
        "/tmp/fm_thumbnail_6.jpg",
    ];

    pub fn new(source: &Arc<Path>, kind: Kind) -> Self {
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
        log_info!("build_office: build starting!");
        let calc_str = path_to_string(&self.source);
        let args = ["--convert-to", "pdf", "--outdir", "/tmp", &calc_str];
        execute_and_output_no_log(LIBREOFFICE, args)?;
        log_info!("build_office: here");
        let mut pdf_path = std::path::PathBuf::from("/tmp");
        let filename = self.source.file_name().context("")?;
        pdf_path.push(filename);
        pdf_path.set_extension("pdf");
        let calc_pdf_path = PathBuf::from(&pdf_path);
        if !pdf_path.exists() {
            bail!("{LIBREOFFICE} couldn't convert {calc_str} to pdf");
        }
        let identifier = filename_from_path(&pdf_path)?.to_owned();
        let length = Self::get_pdf_length(&calc_pdf_path)?;
        Thumbnail::create(&self.kind, pdf_path.to_string_lossy().as_ref())?;
        let images = Self::make_pdf_images_paths(length)?;
        std::fs::remove_file(&pdf_path)?;
        log_info!("build_office: build complete!");
        Ok(Ueber::new(Kind::Pdf, identifier, images, length))
    }

    fn make_pdf_images_paths(length: usize) -> Result<Vec<PathBuf>> {
        let images = (1..length + 1)
            .map(|index| PathBuf::from(format!("{THUMBNAIL_PDF_PATH}-{index}.jpg")))
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
            None => Err(anyhow::Error::msg("Couldn't find the page number")),
        }
    }

    fn build_pdf(self) -> Result<Ueber> {
        let length = Self::get_pdf_length(&self.source)?;
        let identifier = filename_from_path(&self.source)?.to_owned();
        Thumbnail::create(&self.kind, self.source.to_string_lossy().as_ref())?;
        let images = Self::make_pdf_images_paths(length)?;
        log_info!("build_pdf images: {images:?}");
        Ok(Ueber::new(self.kind, identifier, images, length))
    }

    fn build_video(self) -> Result<Ueber> {
        let path_str = self
            .source
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Thumbnail::create(&self.kind, path_str)?;
        let images: Vec<PathBuf> = Self::VIDEO_THUMBNAILS
            .map(PathBuf::from)
            .into_iter()
            .filter(|p| p.exists())
            .collect();
        let identifier = filename_from_path(&self.source)?.to_owned();
        let length = images.len();
        Ok(Ueber::new(self.kind, identifier, images, length))
    }

    fn build_single_image(self, images: Vec<PathBuf>) -> Result<Ueber> {
        let identifier = filename_from_path(&self.source)?.to_owned();
        let length = 1;
        Ok(Ueber::new(self.kind, identifier, images, length))
    }

    fn build_font(self) -> Result<Ueber> {
        let path_str = self
            .source
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Thumbnail::create(&self.kind, path_str)?;
        let images = vec![PathBuf::from(THUMBNAIL_PATH_PNG)];
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
        Thumbnail::create(&self.kind, path_str)?;
        let images = vec![PathBuf::from(THUMBNAIL_PATH_PNG)];
        self.build_single_image(images)
    }
}

struct Thumbnail;

impl Thumbnail {
    fn create(kind: &Kind, path_str: &str) -> Result<()> {
        match kind {
            Kind::Font => Self::create_font(path_str),
            Kind::Office => Self::create_office(path_str),
            Kind::Pdf => Self::create_pdf(path_str),
            Kind::Svg => Self::create_svg(path_str),
            Kind::Video => Self::create_video(path_str),

            _ => Ok(()),
        }
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

    fn create_video(path_str: &str) -> Result<()> {
        let ffmpeg_args = [
            "-i",
            path_str,
            "-vf",
            "fps=1/60",
            "scale=320:-1",
            "-vsync",
            "vfr",
            "-frames:v",
            "6",
            &format!("{THUMBNAIL_PATH_NO_EXT}_%d.jpg"),
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
                THUMBNAIL_PDF_PATH,
            ],
        )
    }

    fn execute(exe: &str, args: &[&str]) -> Result<()> {
        let output = execute_and_output_no_log(exe, args.to_owned())?;
        if !output.stderr.is_empty() {
            log_info!(
                "make thumbnail output: {} {}",
                String::from_utf8(output.stdout).unwrap_or_default(),
                String::from_utf8(output.stderr).unwrap_or_default()
            );
        }
        Ok(())
    }
}
