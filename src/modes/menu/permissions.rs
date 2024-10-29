use std::os::unix::fs::PermissionsExt;

use anyhow::Result;

use crate::log_line;
use crate::modes::{convert_octal_mode, Flagged};

type Mode = u32;

/// Empty struct used to regroup some methods.
pub struct Permissions;

/// Maximum possible mode, 0o777 = 511 (decimal)
pub const MAX_MODE: Mode = 0o777;

impl Permissions {
    /// Set the permissions of the flagged files according to a given permission.
    ///
    /// # Errors
    ///
    /// If the permission are invalid or if the user can't edit them, it may fail.
    fn set_permissions<P>(path: P, mode: Mode) -> std::io::Result<()>
    where
        P: AsRef<std::path::Path>,
    {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    ///
    /// # Errors
    ///
    /// It may fail if the permissions can't be set by the user.
    pub fn set_permissions_of_flagged(mode_str: &str, flagged: &mut Flagged) -> Result<()> {
        let Some(mode) = _Mode::from_str(mode_str) else {
            return Ok(());
        };
        for path in &flagged.content {
            Self::set_permissions(path, mode.octal())?;
        }
        log_line!("Changed permissions to {mode_str}");
        Ok(())
    }
}

type IsValid = bool;

/// Parse an inputstring into a displayed textual permission.
/// Converts `644` into `rw-r--r--` and like so,
/// Converts `944` into `???r--r--` and like so,
/// Converts `66222` into "Mode is too long".
/// It also returns a flag for any char, set to true if the char
/// is a valid permission.
/// It's used to display a valid mode or not.
pub fn parse_input_mode(mode_str: &str) -> Vec<(&'static str, IsValid)> {
    if mode_str.len() > 3 {
        return vec![("Mode is too long", false)];
    }
    let mut display = vec![];
    for char in mode_str.chars() {
        if char.is_digit(8) {
            let mode = convert_octal_mode(char.to_digit(8).unwrap_or_default() as usize);
            display.push((mode, true));
        } else {
            display.push(("???", false));
        }
    }
    display
}

struct _Mode(Mode);

impl _Mode {
    /// Max valid mode, ie `0o777`.

    const fn octal(&self) -> Mode {
        self.0
    }

    fn from_str(mode_str: &str) -> Option<Self> {
        if let Ok(mode) = Mode::from_str_radix(mode_str, 8) {
            if Self::is_valid_permissions(mode) {
                return Some(Self(mode));
            }
        }
        None
    }

    const fn is_valid_permissions(mode: Mode) -> bool {
        mode <= MAX_MODE
    }
}
