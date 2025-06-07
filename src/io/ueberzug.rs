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

use crate::log_info;

/// Check if user has display capabilities.
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
    has_x11: bool,
    driver: Child,
    last_displayed: Option<String>,
    is_displaying: bool,
}

impl Default for Ueberzug {
    /// Creates the Default Ueberzug instance
    /// One instance can handel multiple images provided they have different identifiers
    fn default() -> Self {
        Self {
            has_x11: user_has_x11(),
            driver: Self::spawn_ueberzug().unwrap(),
            last_displayed: None,
            is_displaying: false,
        }
    }
}

impl Ueberzug {
    /// Draws the Image using UeConf
    pub fn draw(&mut self, config: &UeConf) -> Result<()> {
        if !self.change_current_image(config)? {
            let cmd = config.to_json();
            self.is_displaying = true;
            self.run(&cmd)
        } else {
            Ok(())
        }
    }

    /// true iff the same image was already displayed
    /// Doesn't draw the image itself but update the last image value
    /// and clear the last one if it was different.
    /// It's useful to avoid calling ueberzug multiple times to
    /// display the same image at the same place.
    fn change_current_image(&mut self, config: &UeConf) -> Result<bool> {
        let current = config.path;
        if let Some(last) = &self.last_displayed {
            if last != current {
                self.clear(&last.clone())?;
                self.last_displayed = Some(current.to_owned());
                Ok(false)
            } else {
                Ok(self.is_displaying)
            }
        } else {
            self.last_displayed = Some(current.to_owned());
            Ok(false)
        }
    }

    /// Clear the last image.
    pub fn clear_all(&mut self) -> Result<()> {
        if let Some(last_displayed) = &self.last_displayed {
            self.clear(last_displayed.clone().as_str())?;
        }
        Ok(())
    }

    fn spawn_ueberzug() -> std::io::Result<Child> {
        std::process::Command::new("ueberzug")
            .arg("layer")
            // .arg("--silent")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    /// Clear the drawn image only requires the identifier
    pub fn clear(&mut self, identifier: &str) -> Result<()> {
        let config = UeConf::remove(identifier);
        let cmd = config.to_json();
        self.run(&cmd)
    }

    fn run(&mut self, cmd: &str) -> Result<()> {
        if !self.has_x11 {
            log_info!("Can't display since user hasn't x11");
            return Ok(());
        }
        // let mut ueberzug = self.driver.write().unwrap();
        // if ueberzug.is_none() {
        //     log_info!("Spawned a new ueberzug");
        //     *ueberzug = Some(Self::spawn_ueberzug()?);
        // }

        // let mut buf_stdout = vec![];
        // let stdout = (*ueberzug)
        //     .as_mut()
        //     .context("Ueberzug shouldn't be None")?
        //     .stdout
        //     .as_mut()
        //     .context("stdout shouldn't be None")?;
        // stdout.read_to_end(&mut buf_stdout)?;
        // let mut buf_stderr = vec![];
        // let stderr = (*ueberzug)
        //     .as_mut()
        //     .context("Ueberzug shouldn't be None")?
        //     .stderr
        //     .as_mut()
        //     .context("stderr shouldn't be None")?;
        // stderr.read_to_end(&mut buf_stderr)?;
        // log_info!(
        //     "before {out} - {err}",
        //     out = String::from_utf8(buf_stdout).unwrap(),
        //     err = String::from_utf8(buf_stderr).unwrap()
        // );

        let stdin = self
            .driver
            .stdin
            .as_mut()
            .context("stdin shouldn't be None")?;
        stdin.write_all(cmd.as_bytes())?;

        // let mut buf_stdout = vec![];
        // let stdout = (*ueberzug)
        //     .as_mut()
        //     .context("Ueberzug shouldn't be None")?
        //     .stdout
        //     .as_mut()
        //     .context("stdout shouldn't be None")?;
        // stdout.read_to_end(&mut buf_stdout)?;
        // let mut buf_stderr = vec![];
        // let stderr = (*ueberzug)
        //     .as_mut()
        //     .context("Ueberzug shouldn't be None")?
        //     .stderr
        //     .as_mut()
        //     .context("stderr shouldn't be None")?;
        // stderr.read_to_end(&mut buf_stderr)?;
        // log_info!(
        //     "after {out} - {err}",
        //     out = String::from_utf8(buf_stdout).unwrap(),
        //     err = String::from_utf8(buf_stderr).unwrap()
        // );
        Ok(())
    }
}

/// Action enum for the json value
pub enum Actions {
    Add,
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
#[derive(Clone, Copy)]
pub enum Scalers {
    Crop,
    Distort,
    FitContain,
    Contain,
    ForcedCover,
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
pub struct UeConf<'a> {
    pub action: Actions,
    pub identifier: &'a str,
    pub x: u16,
    pub y: u16,
    pub path: &'a str,
    pub width: Option<u16>,
    pub height: Option<u16>,
    pub scaler: Option<Scalers>,
    pub draw: Option<bool>,
    pub synchronously_draw: Option<bool>,
    pub scaling_position_x: Option<f32>,
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

macro_rules! if_not_none {
    ($st:expr,$name:expr,$val:expr) => {
        match $val {
            Some(z) => $st + &format!(",\"{}\":\"{}\"", $name, z),
            None => $st,
        }
    };
}

impl<'a> UeConf<'a> {
    fn remove(identifier: &'a str) -> Self {
        Self {
            action: Actions::Remove,
            identifier,
            ..Default::default()
        }
    }
    fn to_json(&self) -> String {
        if self.identifier.is_empty() {
            panic!("Incomplete Information : Itentifier Not Found");
        }
        match self.action {
            Actions::Add => {
                if self.path.is_empty() {
                    panic!("Incomplete Information : Path empty");
                }
                let mut jsn = String::from(r#"{"action":"add","#);
                jsn = jsn
                    + &format!(
                        "\"path\":\"{}\",\"identifier\":\"{}\",\"x\":{},\"y\":{}",
                        self.path, self.identifier, self.x, self.y
                    );
                jsn = if_not_none!(jsn, "width", self.width);
                jsn = if_not_none!(jsn, "height", self.height);
                jsn = if_not_none!(jsn, "scaler", self.scaler);
                jsn = if_not_none!(jsn, "draw", self.draw);
                jsn = if_not_none!(jsn, "sync", self.synchronously_draw);
                jsn = if_not_none!(jsn, "scaling_position_x", self.scaling_position_x);
                jsn = if_not_none!(jsn, "scaling_position_y", self.scaling_position_y);
                jsn += "}\n";
                jsn
            }
            Actions::Remove => format!(
                "{{\"action\":\"remove\",\"identifier\":\"{}\"}}\n",
                self.identifier
            ),
        }
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
    #[test]
    fn json_conversion() {
        let conf = UeConf {
            identifier: "a",
            path: "a",
            ..Default::default()
        };
        let rem_conf = UeConf {
            action: Actions::Remove,
            identifier: "a",
            ..Default::default()
        };
        assert_eq!(
            conf.to_json(),
            "{\"action\":\"add\",\"path\":\"a\",\"identifier\":\"a\",\"x\":0,\"y\":0}\n"
        );
        assert_eq!(
            rem_conf.to_json(),
            "{\"action\":\"remove\",\"identifier\":\"a\"}\n"
        );
    }
}
