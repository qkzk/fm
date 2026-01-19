//! # Ueberzug-rs
//! [Ueberzug-rs](https://github.com/Adit-Chauhan/Ueberzug-rs) This project provides simple bindings to that [ueberzug](https://github.com/seebye/ueberzug) to draw images in the terminal.
//!
//!This code was inspired from the [termusic](https://github.com/tramhao/termusic) to convert their specilized approach to a more general one.
//!
//! ## Examples
//! this example will draw image for 2 seconds, erase the image and wait 1 second before exiting the program.
//!
//! This code was copied from the above repository which wasn't maintained anymore
//! ```
//! use std::thread::sleep;
//! use std::time::Duration;
//! use ueberzug::{UeConf,Scalers};
//!
//! let a = ueberzug::Ueberzug::new();
//! // Draw image
//! // See UeConf for more details
//! a.draw(&UeConf {
//!     identifier: "crab",
//!     path: "ferris.png",
//!     x: 10,
//!     y: 2,
//!     width: Some(10),
//!     height: Some(10),
//!     scaler: Some(Scalers::FitContain),
//!     ..Default::default()
//! });
//! sleep(Duration::from_secs(2));
//! // Only identifier needed to clear image
//! a.clear("crab");
//! sleep(Duration::from_secs(1));
//! ```

use std::env::var;
use std::fmt;
use std::io::Write;
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};
use ratatui::layout::Rect;
use serde::Serialize;
use serde_json::Result as ResultSerdeJson;

use crate::common::UEBERZUG;
use crate::io::ImageDisplayer;
use crate::modes::{DisplayedImage, Quote};

/// Check if user has X11 display capabilities.
/// Call it before trying to spawn ueberzug.
///
/// Normal session (terminal emulator from X11 window manager) should have:
/// - A "DISPLAY" environment variable set,
/// - no error while running `xset q`, which displays informations about X11 sessions.
///
/// If either of these conditions isn't satisfied, the user can't display with ueberzug.
pub fn user_has_x11() -> bool {
    if var("DISPLAY").is_err() {
        return false;
    }

    Command::new("xset")
        .arg("q")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Main Ueberzug Struct
///
/// If `self.has_x11` is false, nothing will ever be displayed.
/// it prevents ueberzug to crash for nothing, trying to open a session.
pub struct Ueberzug {
    driver: Child,
    last_displayed: Option<String>,
    is_displaying: bool,
}

impl Default for Ueberzug {
    /// Creates the Default Ueberzug instance
    /// One instance can handel multiple images provided they have different identifiers
    fn default() -> Self {
        Self {
            driver: Self::spawn_ueberzug().unwrap(),
            last_displayed: None,
            is_displaying: false,
        }
    }
}

impl ImageDisplayer for Ueberzug {
    /// Draws the Image using ueberzug
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) -> Result<()> {
        let path = image.selected_path().quote()?;

        if self.is_the_same_image(&path) {
            Ok(())
        } else {
            self.clear(image)?;
            self.is_displaying = true;
            self.last_displayed = Some(path.to_string());
            self.run(&UeConf::add_json(image, rect)?)
        }
    }

    /// Clear the drawn image.
    /// Only requires the identifier
    fn clear(&mut self, _: &DisplayedImage) -> Result<()> {
        if self.is_displaying {
            self.clear_internal()
        } else {
            Ok(())
        }
    }

    /// Clear the last image.
    fn clear_all(&mut self) -> Result<()> {
        self.is_displaying = false;
        self.last_displayed = None;
        self.driver = Self::spawn_ueberzug()?;
        Ok(())
    }
}

impl Ueberzug {
    fn clear_internal(&mut self) -> Result<()> {
        self.is_displaying = false;
        self.last_displayed = None;
        self.run(&UeConf::remove_json("fm_tui")?)
    }
    /// true iff the same image was already displayed
    fn is_the_same_image(&mut self, new: &str) -> bool {
        let Some(last) = &self.last_displayed else {
            return false;
        };
        last == new
    }

    fn spawn_ueberzug() -> std::io::Result<Child> {
        std::process::Command::new(UEBERZUG)
            .arg("layer")
            .arg("--silent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn run(&mut self, cmd: &str) -> Result<()> {
        self.driver
            .stdin
            .as_mut()
            .context("stdin shouldn't be None")?
            .write_all(cmd.as_bytes())?;
        self.driver
            .stdin
            .as_mut()
            .context("stdin shouldn't be None")?
            .write_all(b"\n")?;
        Ok(())
    }
}

/// Action enum for the json value
#[derive(Serialize)]
pub enum Actions {
    #[serde(rename(serialize = "add"))]
    Add,
    #[serde(rename(serialize = "remove"))]
    Remove,
}

impl fmt::Display for Actions {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Actions::Add => write!(f, "add"),
            Actions::Remove => write!(f, "remove"),
        }
    }
}
/// Scalers that can be applied to the image and are supported by ueberzug
#[derive(Clone, Copy, Serialize)]
pub enum Scalers {
    #[serde(rename(serialize = "crop"))]
    Crop,
    #[serde(rename(serialize = "distort"))]
    Distort,
    #[serde(rename(serialize = "fit_contain"))]
    FitContain,
    #[serde(rename(serialize = "contain"))]
    Contain,
    #[serde(rename(serialize = "forced_cover"))]
    ForcedCover,
    #[serde(rename(serialize = "cover"))]
    Cover,
}

impl fmt::Display for Scalers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Scalers::Contain => write!(f, "contain"),
            Scalers::Cover => write!(f, "cover"),
            Scalers::Crop => write!(f, "crop"),
            Scalers::Distort => write!(f, "distort"),
            Scalers::FitContain => write!(f, "fit_contain"),
            Scalers::ForcedCover => write!(f, "forced_cover"),
        }
    }
}

/// The configuration struct for the image drawing.
///
/// *identifier* and *path* are the only required fields and will throw a panic if left empty.
///
/// By default *x* and *y* will be set to 0 and all other option will be set to None
///
/// ## Example
/// ```
/// use ueberzug::UeConf;
/// // The minimum required for proper config struct.
/// let conf = UeConf{
///             identifier:"carb",
///             path:"ferris.png",
///             ..Default::default()
///             };
///
/// // More specific option with starting x and y cordinates with width and height
/// let conf = UeConf{
///             identifier:"crab",
///             path:"ferris.png",
///             x:20,
///             y:5,
///             width:Some(30),
///             height:Some(30),
///             ..Default::default()
///             };
///```
#[derive(Serialize)]
pub struct UeConf<'a> {
    pub action: Actions,
    pub path: &'a str,
    pub identifier: &'a str,
    pub x: u16,
    pub y: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaler: Option<Scalers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synchronously_draw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling_position_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaling_position_y: Option<f32>,
}

impl<'a> Default for UeConf<'a> {
    fn default() -> Self {
        Self {
            action: Actions::Add,
            identifier: "",
            x: 0,
            y: 0,
            path: "",
            width: None,
            height: None,
            scaler: None,
            draw: None,
            synchronously_draw: None,
            scaling_position_x: None,
            scaling_position_y: None,
        }
    }
}

impl<'a> UeConf<'a> {
    fn remove_json(identifier: &'a str) -> ResultSerdeJson<String> {
        let config = Self {
            action: Actions::Remove,
            identifier,
            ..Default::default()
        };
        serde_json::to_string(&config)
    }

    fn add_json(image: &DisplayedImage, rect: Rect) -> ResultSerdeJson<String> {
        let path = &image.selected_path();
        let x = rect.x;
        let y = rect.y.saturating_sub(1);
        let width = Some(rect.width);
        let height = Some(rect.height.saturating_sub(1));
        let scaler = Some(Scalers::FitContain);
        let config = UeConf {
            identifier: "fm_tui",
            path,
            x,
            y,
            width,
            height,
            scaler,
            ..Default::default()
        };

        serde_json::to_string(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enum_to_str() {
        let add = Actions::Add;
        let remove = Actions::Remove;
        assert_eq!(add.to_string(), "add");
        assert_eq!(format!("{}", remove), "remove");
        let scaler_1 = Scalers::Contain;
        let scaler_2 = Scalers::FitContain;
        assert_eq!(scaler_1.to_string(), "contain");
        assert_eq!(scaler_2.to_string(), "fit_contain");
    }
}
