use anyhow::Result;
use ratatui::layout::Rect;

use crate::config::{get_prefered_imager, Imagers};
use crate::io::{Chafa, InlineImage, Ueberzug};
use crate::modes::DisplayedImage;

pub const COMPATIBLES: [&str; 4] = [
    "WEZTERM_EXECUTABLE",
    "WARP_HONOR_PS1",
    "TABBY_CONFIG_DIRECTORY",
    "VSCODE_INJECTION",
];

/// What image adapter is used ?
/// - Unable means no supported image adapter. ie. image can't be displayed.
/// - Ueberzug if it's installed,
/// - InlineImage if the terminal emulator supports it.
#[derive(Default)]
pub enum ImageAdapter {
    #[default]
    Unable,
    Ueberzug(Ueberzug),
    InlineImage(InlineImage),
    Chafa(Chafa),
}

impl From<&Imagers> for ImageAdapter {
    fn from(imager: &Imagers) -> Self {
        match imager {
            Imagers::Disabled => Self::Unable,
            Imagers::Inline => Self::InlineImage(InlineImage::default()),
            Imagers::Chafa => Self::Chafa(Chafa::default()),
            Imagers::Ueberzug => Self::Ueberzug(Ueberzug::default()),
        }
    }
}

impl ImageAdapter {
    /// Returns a compatible `ImageAdapter` from configuration.
    pub fn detect() -> Self {
        let Some(prefered_imager) = get_prefered_imager() else {
            return Self::Unable;
        };

        (&prefered_imager.imager).into()
    }
}

/// Methods used to display images :
/// - `draw` asks the adapter to do the drawing,
/// - `clear` erases an image from its path,
/// - `clear_all` erases all drawed images.
pub trait ImageDisplayer {
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) -> Result<()>;
    fn clear(&mut self, image: &DisplayedImage) -> Result<()>;
    fn clear_all(&mut self) -> Result<()>;
}

impl ImageDisplayer for ImageAdapter {
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) -> Result<()> {
        match self {
            Self::Unable => Ok(()),
            Self::Ueberzug(ueberzug) => ueberzug.draw(image, rect),
            Self::InlineImage(inline_image) => inline_image.draw(image, rect),
            Self::Chafa(chafa) => chafa.draw(image, rect),
        }
    }

    fn clear(&mut self, image: &DisplayedImage) -> Result<()> {
        match self {
            Self::Unable => Ok(()),
            Self::Ueberzug(ueberzug) => ueberzug.clear(image),
            Self::InlineImage(inline_image) => inline_image.clear(image),
            Self::Chafa(chafa) => chafa.clear(image),
        }
    }

    fn clear_all(&mut self) -> Result<()> {
        match self {
            Self::Unable => Ok(()),
            Self::Ueberzug(ueberzug) => ueberzug.clear_all(),
            Self::InlineImage(inline_image) => inline_image.clear_all(),
            Self::Chafa(chafa) => chafa.clear_all(),
        }
    }
}
