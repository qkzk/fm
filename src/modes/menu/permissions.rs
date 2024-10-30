use std::os::unix::fs::PermissionsExt;

use anyhow::Result;

use crate::modes::{convert_octal_mode, Flagged};
use crate::{log_info, log_line};

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
pub fn parse_input_mode(mode_str: &str) -> Vec<(&str, IsValid)> {
    if mode_str.chars().any(|c| c.is_alphabetic()) {
        return vec![("", true)];
    }
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
    const VALID: [char; 3] = ['r', 'w', 'x'];
    const ACCEPTED: [char; 2] = ['.', '-'];
    /// Max valid mode, ie `0o777`.

    const fn octal(&self) -> Mode {
        self.0
    }

    fn from_str(mode_str: &str) -> Option<Self> {
        if let Some(mode) = Self::from_numeric(mode_str) {
            return Some(mode);
        }
        Self::from_alphabetic(mode_str)
    }

    fn from_numeric(mode_str: &str) -> Option<Self> {
        if let Ok(mode) = Mode::from_str_radix(mode_str, 8) {
            if Self::is_valid_permissions(mode) {
                return Some(Self(mode));
            }
        }
        None
    }

    /// Convert a 9 char len long string into a mode.
    /// It will only accept basic strings like "rw.r...." or "rw-r----".
    /// It won't accept specific chmod syntax like a+x or +X or s or t.
    /// User can execute a command like !chmod a+x %s and use chmod directly.
    fn from_alphabetic(mode_str: &str) -> Option<Self> {
        // rwxrwxrwx
        if mode_str.len() != 9 {
            return None;
        }
        let mut exponent;
        let mut current_index: usize;
        let mut current_char: char;
        let mut mode: u32 = 0;

        let chars: Vec<_> = mode_str.chars().collect();
        for part in 0..3 {
            mode <<= 3;
            exponent = 4;
            for (index, valid) in Self::VALID.iter().enumerate() {
                current_index = part * 3 + index;
                current_char = chars[current_index];
                if current_char == *valid {
                    mode += exponent;
                } else if current_char != Self::ACCEPTED[0] && current_char != Self::ACCEPTED[1] {
                    log_info!("Invalid char in permissions {current_char}");
                    return None;
                }
                exponent >>= 1;
            }
        }
        if Self::is_valid_permissions(mode) {
            return Some(Self(mode));
        }

        None
    }

    const fn is_valid_permissions(mode: Mode) -> bool {
        mode <= MAX_MODE
    }
}
