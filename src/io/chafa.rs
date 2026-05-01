use std::io::{stdout, Write};

use anyhow::{Context, Result};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::layout::Rect;

use crate::{common::CHAFA, io::ImageDisplayer};
use crate::{io::execute_and_capture_output_with_path, modes::DisplayedImage};

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

    /// true iff the displayed image path and its rect haven't changed
    fn is_same(&self, path: &str, rect: Rect) -> bool {
        self.path == path && self.rect == rect
    }
}

/// Which image was displayed, where on the screen and is it displayed ?
#[derive(Default, Debug)]
pub struct Chafa {
    last_displayed: Option<PathRect>,
    is_displaying: bool,
}

impl ImageDisplayer for Chafa {
    /// Draws the image to the terminal using [chafa](<https://hpjansson.org/chafa/>).
    ///
    /// The drawing is done using the first method supported by the terminal (iterm2, kitty, sixel or symbols).
    /// It requires a string to be "written" to the terminal itself.
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) -> Result<()> {
        let path = &image.selected_path();
        if self.image_can_be_reused(path, rect) {
            return Ok(());
        }
        let image_string = Self::encode_chafa(path, rect)?;
        let image_encoded = image_string.as_bytes();
        Self::write_image_to_term(image_encoded, rect)?;
        self.is_displaying = true;
        self.last_displayed = Some(PathRect::new(path.to_string(), rect));
        Ok(())
    }

    /// Clear the last displayed image.
    /// Alias to clear_all.
    ///
    /// If an image is currently displayed, write lines of " " in all its rect.
    fn clear(&mut self, _: &DisplayedImage) -> Result<()> {
        self.clear_all()
    }

    /// Clear the last displayed image.
    /// If an image is currently displayed, write lines of " " in all its rect.
    fn clear_all(&mut self) -> Result<()> {
        if let Some(PathRect { path: _, rect }) = self.last_displayed {
            Self::clear_image_rect(rect)?;
        }
        self.is_displaying = false;
        self.last_displayed = None;
        Ok(())
    }
}

impl Chafa {
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

    /// Encode an image to a string using iterm2 inline image protocol.
    fn encode_chafa<P>(path: P, rect: Rect) -> Result<String>
    where
        P: AsRef<str>,
    {
        Self::write_chafa(path.as_ref(), rect.width, rect.height)
    }

    /// To draw an image on the terminal using Inline Image Protocol,
    /// We must :
    /// - disable raw mode,
    /// - move to the position,
    /// - write the encoded bytes to stdout,
    /// - enable raw mode.
    ///
    /// Heavily inspired by Yazi.
    fn write_image_to_term(encoded_image: &[u8], rect: Rect) -> std::io::Result<()> {
        disable_raw_mode()?;
        execute!(stdout(), MoveTo(rect.x, rect.y))?;
        stdout().write_all(encoded_image)?;
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

    /// Creates the chafa string. We force a view-size of the surrounding rect and ensure "relative" is on. It allows
    /// the image to be displayed properly in its position.
    ///
    /// The resizing must be done by the terminal emulator itself.
    fn write_chafa(path: &str, width: u16, height: u16) -> Result<String> {
        let output = execute_and_capture_output_with_path(
            CHAFA,
            std::path::Path::new(path)
                .parent()
                .context("no parent of image path")?,
            &[
                "--view-size",
                &format!("{width}x{height}"),
                "--relative",
                "on",
                path,
            ],
        )?;

        Ok(output)
    }
}
