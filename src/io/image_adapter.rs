use std::env::var;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::common::{is_in_path, UEBERZUG};
use crate::io::{user_has_x11, InlineImage, Ueberzug};
use crate::log_info;
use crate::modes::DisplayedImage;

const COMPATIBLES: [&str; 4] = [
    "WEZTERM_EXECUTABLE",
    "WARP_HONOR_PS1",
    "TABBY_CONFIG_DIRECTORY",
    "VSCODE_INJECTION",
];

#[derive(Default)]
pub enum ImageAdapter {
    Ueberzug(Ueberzug),
    Iterm2(InlineImage),
    #[default]
    Unable,
}

impl ImageAdapter {
    /// Returns a compatible `ImageAdapter` from environement and installed programs.
    /// We first look for some terminal emulators variable set at launch.
    /// If we detect a "Inline Images Protocol compatible", we use it.
    /// Else, we check for the executable ueberzug and the X11 capacity,
    /// Else we can't display the image.
    pub fn detect() -> Self {
        for variable in COMPATIBLES {
            if var(variable).is_ok() {
                log_info!("iterm2");
                return Self::Iterm2(InlineImage::default());
            }
        }
        if is_in_path(UEBERZUG) && user_has_x11() {
            log_info!("ueberzug");
            Self::Ueberzug(Ueberzug::default())
        } else {
            log_info!("unable to display image");
            Self::Unable
        }
    }
}

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
            Self::Iterm2(inline_image) => inline_image.draw(image, rect),
        }
    }

    fn clear(&mut self, image: &DisplayedImage) -> Result<()> {
        match self {
            Self::Unable => Ok(()),
            Self::Ueberzug(ueberzug) => ueberzug.clear(image),
            Self::Iterm2(inline_image) => inline_image.clear(image),
        }
    }

    fn clear_all(&mut self) -> Result<()> {
        match self {
            Self::Unable => Ok(()),
            Self::Ueberzug(ueberzug) => ueberzug.clear_all(),
            Self::Iterm2(inline_image) => inline_image.clear_all(),
        }
    }
}
