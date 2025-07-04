use std::{
    fmt::Write as FmtWrite,
    fs::File,
    io::{stdout, Read, Write},
};

use anyhow::Result;
use base64::{
    encoded_len as base64_encoded_len,
    engine::{general_purpose::STANDARD, Config},
    Engine,
};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::layout::Rect;

use crate::io::ImageDisplayer;
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

    /// true iff the displayed image path and its rect haven't changed
    fn is_same(&self, path: &str, rect: Rect) -> bool {
        self.path == path && self.rect == rect
    }
}

/// Which image was displayed, where on the screen and is it displayed ?
#[derive(Default, Debug)]
pub struct InlineImage {
    last_displayed: Option<PathRect>,
    is_displaying: bool,
}

impl ImageDisplayer for InlineImage {
    /// Draws the image to the terminal using [iterm2 Inline Image Protocol](https://iterm2.com/documentation-images.html).
    ///
    /// The drawing itself is done by the terminal emulator.
    /// It requires a string to be "written" to the terminal itself which will parse it and display the image.
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) -> Result<()> {
        let path = &image.selected_path();
        if self.image_can_be_reused(path, rect) {
            return Ok(());
        }
        let image_string = Self::encode_to_string(path, rect)?;
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
    fn read_as_bytes<P>(path: P) -> std::io::Result<Vec<u8>>
    where
        P: AsRef<str>,
    {
        let mut f = File::open(path.as_ref())?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Encode an image to a string using iterm2 inline image protocol.
    fn encode_to_string<P>(path: P, rect: Rect) -> Result<String>
    where
        P: AsRef<str>,
    {
        Self::write_inline_image_string(
            &Self::read_as_bytes(path)?,
            rect.width.saturating_sub(1),
            rect.height.saturating_sub(4),
        )
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

    /// Creates the [iterm2 Inline Image Protocol string](https://iterm2.com/documentation-images.html)
    /// It sets an image size, a cell width, a cell height, doNotMoveCursor to 1 and preserveAspectRatio to 1.
    ///
    /// The resizing must be done by the terminal emulator itself.
    /// For [WezTerm](https://wezterm.org/) it's faster this way. Hasn't been tested on other terminal emulator.
    fn write_inline_image_string(buffer: &[u8], width: u16, height: u16) -> Result<String> {
        let mut string = String::with_capacity(Self::guess_string_capacity(buffer));
        write!(
            string,
            "\x1b]1337;File=inline=1;size={size};width={width};height={height};doNotMoveCursor=1;preserveAspectRatio=1:",
            size = buffer.len(),
        )?;
        STANDARD.encode_string(buffer, &mut string);
        write!(string, "\u{0007}")?;
        Ok(string)
    }

    fn guess_string_capacity(buffer: &[u8]) -> usize {
        200 + base64_encoded_len(buffer.len(), STANDARD.config().encode_padding()).unwrap_or(0)
    }
}
