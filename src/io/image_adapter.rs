use std::env::var;
use std::path::Path;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::common::{is_in_path, UEBERZUG};
use crate::io::user_has_x11;

const COMPATIBLES: [&str; 4] = [
    "WEZTERM_EXECUTABLE",
    "WARP_HONOR_PS1",
    "TABBY_CONFIG_DIRECTORY",
    "VSCODE_INJECTION",
];

#[derive(Default)]
pub enum ImageAdapter {
    #[default]
    Ueberzug,
    Iterm2,
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
                return Self::Iterm2;
            }
        }
        if is_in_path(UEBERZUG) && user_has_x11() {
            Self::Ueberzug
        } else {
            Self::Unable
        }
    }
}

pub trait ImageDisplayer {
    fn draw(&self, identifier: impl AsRef<Path>, rect: Rect) -> Result<()>;
    fn clear(&self, identifier: impl AsRef<Path>) -> Result<()>;
    fn clear_all(&self);
}

impl ImageDisplayer for ImageAdapter {
    fn draw(&self, identifier: impl AsRef<Path>, rect: Rect) -> Result<()> {
        match &self {
            Self::Unable => (),
            Self::Ueberzug => (),
            Self::Iterm2 => (),
        };
        Ok(())
    }

    fn clear(&self, identifier: impl AsRef<Path>) -> Result<()> {
        match &self {
            Self::Unable => (),
            Self::Ueberzug => (),
            Self::Iterm2 => (),
        };
        Ok(())
    }

    fn clear_all(&self) {
        match &self {
            Self::Unable => (),
            Self::Ueberzug => (),
            Self::Iterm2 => (),
        };
    }
}
