use std::env::var;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::common::{is_in_path, UEBERZUG};
use crate::config::{get_prefered_imager, Imagers};
use crate::io::{user_has_x11, InlineImage, Ueberzug};
use crate::log_info;
use crate::modes::DisplayedImage;

const COMPATIBLES: [&str; 4] = [
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
}

impl ImageAdapter {
    /// Returns a compatible `ImageAdapter` from environement and installed programs.
    /// We first look for some terminal emulators variable set at launch.
    /// If we detect a "Inline Images Protocol compatible", we use it.
    /// Else, we check for the executable ueberzug and the X11 capacity,
    /// Else we can't display the image.
    pub fn detect() -> Self {
        let Some(prefered_imager) = get_prefered_imager() else {
            return Self::Unable;
        };

        // TODO: refactor & simplify
        match prefered_imager.imager {
            Imagers::Disabled => Self::Unable,
            Imagers::Inline => {
                for variable in COMPATIBLES {
                    if var(variable).is_ok() {
                        log_info!(
                            "detected Inline Image Protocol compatible terminal from {variable}"
                        );
                        return Self::InlineImage(InlineImage::default());
                    }
                }
                Self::try_ueberzug()
            }
            Imagers::Ueberzug => Self::try_ueberzug(),
        }
    }

    fn try_ueberzug() -> Self {
        if is_in_path(UEBERZUG) && user_has_x11() {
            log_info!("detected ueberzug");
            Self::Ueberzug(Ueberzug::default())
        } else {
            log_info!("unable to display image");
            Self::Unable
        }
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
        }
    }

    fn clear(&mut self, image: &DisplayedImage) -> Result<()> {
        match self {
            Self::Unable => Ok(()),
            Self::Ueberzug(ueberzug) => ueberzug.clear(image),
            Self::InlineImage(inline_image) => inline_image.clear(image),
        }
    }

    fn clear_all(&mut self) -> Result<()> {
        match self {
            Self::Unable => Ok(()),
            Self::Ueberzug(ueberzug) => ueberzug.clear_all(),
            Self::InlineImage(inline_image) => inline_image.clear_all(),
        }
    }
}
