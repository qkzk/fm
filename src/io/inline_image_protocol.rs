use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{stdout, BufReader, Read, Write};

use anyhow::Result;
use base64::{
    engine::{general_purpose::STANDARD, Config},
    Engine,
};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    imageops::FilterType,
    DynamicImage, ExtendedColorType, ImageEncoder, ImageReader,
};
use ratatui::layout::Rect;

use crate::io::ImageDisplayer;
use crate::log_info;
use crate::modes::DisplayedImage;

/// Holds the path of the image and a rect surrounding its display position.
/// It's used to:
/// - avoid drawing the same image over and over,
/// - know where to draw the new image,
/// - know where to erase the last image.
#[derive(Debug)]
struct PathRect {
    path: String,
    rect: Rect,
}

impl PathRect {
    fn new(path: String, rect: Rect) -> Self {
        Self { path, rect }
    }

    fn is_same(&self, path: &str, rect: Rect) -> bool {
        self.path == path && self.rect == rect
    }
}

#[derive(Default, Debug)]
pub struct InlineImage {
    last_displayed: Option<PathRect>,
    is_displaying: bool,
}

impl ImageDisplayer for InlineImage {
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) -> Result<()> {
        let path = &image.selected_path();
        if self.image_can_be_reused(path, rect) {
            return Ok(());
        }
        let data = self.read_image(image, rect)?;
        // let string = Self::encode_to_string(path, rect)?;
        let encoded = data.as_bytes();
        log_info!("inline image: draws {path} {rect}");
        Self::write_image_to_term(encoded, rect)?;
        self.is_displaying = true;
        self.last_displayed = Some(PathRect::new(path.to_string(), rect));
        Ok(())
    }

    fn clear(&mut self, _: &DisplayedImage) -> Result<()> {
        log_info!("inline image clear {last:?}", last = self.last_displayed);
        if let Some(PathRect { path: _, rect }) = self.last_displayed {
            Self::clear_image_rect(rect)?;
            log_info!("inline image done clearing");
        }
        self.is_displaying = false;
        self.last_displayed = None;
        Ok(())
    }

    fn clear_all(&mut self) -> Result<()> {
        log_info!(
            "inline image clear ALL - last {last:?}",
            last = self.last_displayed
        );
        if let Some(PathRect { path: _, rect }) = self.last_displayed {
            Self::clear_image_rect(rect)?;
            log_info!("inline image done clearing ALL");
        }
        self.is_displaying = false;
        self.last_displayed = None;
        Ok(())
    }
}

impl InlineImage {
    /// True iff the image already drawned can be reused.
    /// Two conditions must be true:
    /// - we are displaying something (is_displaying is true)
    /// - the image itself and its position haven't changed (path and rect haven't changed)
    fn image_can_be_reused<P>(&self, path: P, rect: Rect) -> bool
    where
        P: AsRef<str>,
    {
        if !self.is_displaying {
            return false;
        }
        if let Some(path_rect) = &self.last_displayed {
            path_rect.is_same(path.as_ref(), rect)
        } else {
            false
        }
    }

    /// Read a file from its path to a vector of bytes.
    fn read_as_bytes<P>(path: P) -> Result<Vec<u8>>
    where
        P: AsRef<str>,
    {
        let mut f = File::open(path.as_ref())?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Encode an image to a string using `iterm2img`.
    fn encode_to_string<P>(path: P, rect: Rect) -> Result<String>
    where
        P: AsRef<str>,
    {
        Ok(iterm2img::from_bytes(Self::read_as_bytes(path)?)
            .width(rect.width as u64)
            .height(rect.height as u64)
            .preserve_aspect_ratio(true)
            .inline(true)
            .build())
    }

    /// To draw an image on the terminal using Inline Image Protocol,
    /// We must :
    /// - disable raw mode,
    /// - save position of cursor
    /// - move to the position,
    /// - write the encoded bytes to stdout,
    /// - restore the position of cursor
    /// - enable raw mode.
    ///
    /// Heavily inspired by Yazi.
    fn write_image_to_term(encoded: &[u8], rect: Rect) -> std::io::Result<()> {
        disable_raw_mode()?;
        execute!(stdout(), SavePosition, MoveTo(rect.x, rect.y))?;
        stdout().write_all(encoded)?;
        execute!(stdout(), RestorePosition)?;
        enable_raw_mode()
    }

    /// Clear the rect where the last image were drawned.
    /// Simply write `height` empty lines of length `width`.
    fn clear_image_rect(rect: Rect) -> std::io::Result<()> {
        let empty_line = " ".repeat(rect.width as usize);
        let empty_bytes = empty_line.as_bytes();
        disable_raw_mode()?;
        execute!(stdout(), SavePosition)?;
        for y in rect.top()..rect.bottom() {
            execute!(stdout(), MoveTo(rect.x, y))?;
            stdout().write_all(empty_bytes)?;
        }
        execute!(stdout(), RestorePosition)?;
        enable_raw_mode()
    }

    fn read_image(&mut self, image: &DisplayedImage, rect: Rect) -> Result<String> {
        let reader = ImageReader::open(image.selected_path().as_ref())?;
        let img_read = Self::resize_image(reader, rect)?;
        let width = img_read.width();
        let height = img_read.height();
        let buffer = Self::encode_image(img_read, width, height)?;
        let string = Self::write_inline_image_string(&buffer, width, height)?;
        Ok(string)
    }

    fn rect_pixel_size(rect: Rect) -> Option<(u32, u32)> {
        let window_size = crossterm::terminal::window_size().ok()?;
        if window_size.width == 0 || window_size.height == 0 {
            return None;
        }
        let avail_width =
            (rect.width as u64 * window_size.width as u64 / window_size.columns as u64) as u32;
        let avail_height =
            (rect.height as u64 * window_size.height as u64 / window_size.rows as u64) as u32;
        log_info!("crossterm windowsize {window_size:?} - avail {avail_width} x {avail_height}");
        Some((avail_width, avail_height))
    }

    fn resize_image(reader: ImageReader<BufReader<File>>, rect: Rect) -> Result<DynamicImage> {
        let decoder = reader.with_guessed_format()?.into_decoder()?;
        let mut img_read = DynamicImage::from_decoder(decoder)?;
        let Some((avail_width, avail_height)) = Self::rect_pixel_size(rect) else {
            return Ok(img_read);
        };
        if img_read.width() > avail_width || img_read.height() > avail_height {
            let old_w = img_read.width();
            let old_h = img_read.height();
            img_read = img_read.resize(avail_width, avail_height, FilterType::Nearest);
            log_info!(
                "img resized from {old_w}x{old_h} to {w}x{h} vs avail {avail_width}x{avail_height}",
                w = img_read.width(),
                h = img_read.height()
            );
        }
        Ok(img_read)
    }

    fn encode_image(img_read: DynamicImage, width: u32, height: u32) -> Result<Vec<u8>> {
        let mut buffer = vec![];
        if img_read.color().has_alpha() {
            PngEncoder::new(&mut buffer).write_image(
                &img_read.into_rgba8(),
                width,
                height,
                ExtendedColorType::Rgba8,
            )?;
        } else {
            JpegEncoder::new_with_quality(&mut buffer, 50).encode_image(&img_read)?;
        };
        Ok(buffer)
    }

    fn write_inline_image_string(buffer: &[u8], width: u32, height: u32) -> Result<String> {
        let mut string = String::with_capacity(
            200 + base64::encoded_len(buffer.len(), STANDARD.config().encode_padding())
                .unwrap_or(0),
        );
        write!(
            string,
            "\x1b]1337;File=inline=1;size={size};width={width}px;height={height}px;doNotMoveCursor=1:",
            size = buffer.len(),
        )?;
        STANDARD.encode_string(buffer, &mut string);
        write!(string, "\u{0007}")?;
        Ok(string)
    }
}
