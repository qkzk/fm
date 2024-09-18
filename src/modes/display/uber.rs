use anyhow::{anyhow, Context, Result};

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::common::{
    filename_from_path, path_to_string, CALC_PDF_PATH, FFMPEG, FONTIMAGE, LIBREOFFICE, PDFINFO,
    PDFTOPPM, RSVG_CONVERT, THUMBNAIL_PATH_NO_EXT, THUMBNAIL_PATH_PNG, THUMBNAIL_PDF_PATH,
};
use crate::io::{
    execute_and_capture_output, execute_and_capture_output_without_check, execute_and_output_no_log,
};
use crate::log_info;

pub enum Kind {
    Font,
    Image,
    Office,
    Pdf,
    Svg,
    Video,
}

pub struct Ueber {
    kind: Kind,
    source: PathBuf,
    identifier: String,
    images: Vec<PathBuf>,
    length: usize,
    pub index: usize,
    ueberzug: ueberzug::Ueberzug,
}

impl Ueber {
    fn new(
        kind: Kind,
        source: PathBuf,
        identifier: String,
        images: Vec<PathBuf>,
        length: usize,
        index: usize,
    ) -> Self {
        let ueberzug = ueberzug::Ueberzug::new();
        Self {
            kind,
            source,
            identifier,
            images,
            length,
            index,
            ueberzug,
        }
    }
    /// Only affect pdf thumbnail. Will decrease the index if possible.
    pub fn up_one_row(&mut self) {
        if let Kind::Pdf = self.kind {
            if self.index > 0 {
                self.index -= 1;
            }
        }
    }

    /// Only affect pdf thumbnail. Will increase the index if possible.
    pub fn down_one_row(&mut self) {
        if let Kind::Pdf = self.kind {
            if self.index + 1 < self.len() {
                self.index += 1;
            }
        }
    }

    /// 0 for every kind except pdf where it's the number of pages.
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
            path: &self.images[self.index].to_string_lossy(),
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
        "/tmp/fm_thumbnail1.jpg",
        "/tmp/fm_thumbnail2.jpg",
        "/tmp/fm_thumbnail3.jpg",
        "/tmp/fm_thumbnail4.jpg",
        "/tmp/fm_thumbnail5.jpg",
        "/tmp/fm_thumbnail6.jpg",
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
        }
    }

    fn build_office(self) -> Result<Ueber> {
        log_info!("build_office: build starting!");
        let calc_str = path_to_string(&self.source);
        let args = ["--convert-to", "pdf", "--outdir", "/tmp", &calc_str];
        let output = execute_and_output_no_log(LIBREOFFICE, args)?;
        log_info!("build_office: here");
        // if !output.stderr.is_empty() {
        //     log_info!(
        //         "libreoffice conversion output: {} {}",
        //         String::from_utf8(output.stdout).unwrap_or_default(),
        //         String::from_utf8(output.stderr).unwrap_or_default()
        //     );
        //     return {
        //         Err(anyhow!("{LIBREOFFICE} couldn't convert {calc_str} to pdf"))
        //     }
        //     ;
        // }
        let mut pdf_path = std::path::PathBuf::from("/tmp");
        let filename = self.source.file_name().context("")?;
        pdf_path.push(filename);
        pdf_path.set_extension("pdf");
        std::fs::rename(&pdf_path, CALC_PDF_PATH)?;
        let calc_pdf_path = PathBuf::from(CALC_PDF_PATH);
        let identifier = filename_from_path(&pdf_path)?.to_owned();
        let length = Self::get_pdf_length(&calc_pdf_path)?;
        Self::make_pdf_thumbnails(&calc_pdf_path)?;
        let images = Self::make_pdf_images_paths(length)?;
        log_info!("build_office: build complete!");
        Ok(Ueber::new(
            Kind::Pdf,
            self.source,
            identifier,
            images,
            length,
            0,
        ))
    }

    fn make_pdf_thumbnails(path: &Path) -> Result<String> {
        execute_and_capture_output_without_check(
            PDFTOPPM,
            &[
                "-jpeg",
                "-jpegopt",
                "quality=75",
                path.to_string_lossy().to_string().as_ref(),
                THUMBNAIL_PDF_PATH,
            ],
        )
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
        Self::make_pdf_thumbnails(&self.source)?;
        let images = Self::make_pdf_images_paths(length)?;
        log_info!("build_pdf images: {images:?}");
        Ok(Ueber::new(
            Kind::Pdf,
            self.source,
            identifier,
            images,
            length,
            0,
        ))
    }

    fn build_video(self) -> Result<Ueber> {
        let path_str = self
            .source
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::create_thumbnail(
            FFMPEG,
            &[
                "-i",
                path_str,
                "-vf",
                "\"select='not(mod(n\\,floor(t/6)))',scale=320:-1\"",
                "thumbnail",
                "-vsync",
                "vfr",
                "-frames:v",
                "6",
                "1",
                &format!("{THUMBNAIL_PATH_NO_EXT}%d.jpg"),
                "-y",
            ],
        )?;
        let images = Self::VIDEO_THUMBNAILS
            .map(PathBuf::from)
            .into_iter()
            .collect();
        let identifier = filename_from_path(&self.source)?.to_owned();
        let length = 6;
        let index = 0;
        Ok(Ueber::new(
            self.kind,
            self.source,
            identifier,
            images,
            length,
            index,
        ))
    }

    fn build_single_image(self, images: Vec<PathBuf>) -> Result<Ueber> {
        let identifier = filename_from_path(&self.source)?.to_owned();
        let length = 1;
        let index = 0;
        Ok(Ueber::new(
            self.kind,
            self.source,
            identifier,
            images,
            length,
            index,
        ))
    }

    fn build_font(self) -> Result<Ueber> {
        let path_str = self
            .source
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::create_thumbnail(FONTIMAGE, &["-o", THUMBNAIL_PATH_PNG, path_str])?;
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
        Self::create_thumbnail(
            RSVG_CONVERT,
            &["--keep-aspect-ratio", path_str, "-o", THUMBNAIL_PATH_PNG],
        )?;
        let images = vec![PathBuf::from(THUMBNAIL_PATH_PNG)];
        self.build_single_image(images)
    }

    fn create_thumbnail(exe: &str, args: &[&str]) -> Result<()> {
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
