use std::env::var;

use anyhow::Result;
use ratatui::layout::Rect;

use crate::common::{is_in_path, UEBERZUG};
use crate::io::{user_has_x11, Scalers, UeConf, Ueberzug};
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
    Iterm2,
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
                return Self::Iterm2;
            }
        }
        if is_in_path(UEBERZUG) && user_has_x11() {
            Self::Ueberzug(Ueberzug::default())
        } else {
            Self::Unable
        }
    }
}

pub trait ImageDisplayer {
    fn draw(&mut self, image: &DisplayedImage, rect: Rect);
    fn clear(&mut self, image: &DisplayedImage) -> Result<()>;
    fn clear_all(&mut self) -> Result<()>;
}

impl ImageDisplayer for ImageAdapter {
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) {
        match self {
            Self::Unable => (),
            Self::Ueberzug(ueberzug) => {
                let path = &image.images[image.image_index()].to_string_lossy();
                let x = rect.x;
                let y = rect.y.saturating_sub(1);
                let width = Some(rect.width);
                let height = Some(rect.height.saturating_sub(1));
                let scaler = Some(Scalers::FitContain);
                let config = &UeConf {
                    identifier: "fm_tui",
                    path,
                    x,
                    y,
                    width,
                    height,
                    scaler,
                    ..Default::default()
                };

                if let Err(e) = ueberzug.draw(config) {
                    log_info!(
                        "Ueberzug could not draw {}, from path {}.\n{e}",
                        image.identifier,
                        path
                    );
                };
            }
            Self::Iterm2 => (),
        }
    }

    fn clear(&mut self, _image: &DisplayedImage) -> Result<()> {
        match self {
            Self::Unable => (),
            Self::Ueberzug(ueberzug) => {
                if let Err(e) = ueberzug.clear("fm_tui") {
                    log_info!("Ueberzug could not clear image.\n{e}",);
                };
            }
            Self::Iterm2 => (),
        };
        Ok(())
    }

    fn clear_all(&mut self) -> Result<()> {
        match self {
            Self::Unable => (),
            Self::Ueberzug(ueberzug) => {
                if let Err(e) = ueberzug.clear_all() {
                    log_info!("Ueberzug could not clear image.\n{e}",);
                };
            }
            Self::Iterm2 => (),
        };
        Ok(())
    }
}
