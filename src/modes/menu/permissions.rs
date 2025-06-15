use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use anyhow::Result;

use crate::io::execute_without_output;
use crate::modes::{permission_mode_to_str, Flagged};
use crate::{log_info, log_line};

type Mode = u32;

/// Empty struct used to regroup some methods.
pub struct Permissions;

/// Maximum possible mode for a file, ignoring special bits, 0o777 = 511 (decimal), aka "rwx".
pub const MAX_FILE_MODE: Mode = 0o777;
pub const MAX_SPECIAL_MODE: Mode = 0o7777;

impl Permissions {
    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    ///
    /// # Errors
    ///
    /// It may fail if the permissions can't be set by the user.
    pub fn set_permissions_of_flagged(mode_str: &str, flagged: &Flagged) -> Result<()> {
        log_info!("set_permissions_of_flagged mode_str {mode_str}");
        if let Some(mode) = ModeParser::from_str(mode_str) {
            for path in &flagged.content {
                std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode.numeric()))?;
            }
            log_line!("Changed permissions to {mode_str}");
        } else if Self::validate_chmod_args(mode_str) {
            Self::execute_chmod_for_flagged(mode_str, flagged)?;
        }
        Ok(())
    }

    /// True if a `mode_str` is a valid chmod argument.
    /// This function only validates input in the form "a+x" or "-r"
    ///
    /// The length should be 2 or 3.
    /// If any, the first char should be a, g, o or u
    /// The second char should be + or -
    /// The third char should be X r s t w x
    fn validate_chmod_args(mode_str: &str) -> bool {
        let chars: Vec<_> = mode_str.chars().collect();
        match chars.len() {
            3 => {
                let (dest, action, permission) = (chars[0], chars[1], chars[2]);
                Self::validate_chmod_3(dest, action, permission)
            }
            2 => {
                let (action, permission) = (chars[0], chars[1]);
                Self::validate_chmod_2(action, permission)
            }
            _ => {
                log_info!("{mode_str} isn't a valid chmod argument. Length should be 2 or 3.");
                false
            }
        }
    }

    fn validate_chmod_3(dest: char, action: char, permission: char) -> bool {
        if !"agou".contains(dest) {
            log_info!("{dest} isn't a valid chmod argument. The first char should be 'a', 'g', 'o' or 'u'.");
            return false;
        }
        Self::validate_chmod_2(action, permission)
    }

    fn validate_chmod_2(action: char, permission: char) -> bool {
        if !"+-".contains(action) {
            log_info!(
                "{action} isn't a valid chmod argument. The second char should be '+' or '-'."
            );
            return false;
        }
        if !"XrstwxT".contains(permission) {
            log_info!("{permission} isn't a valid chmod argument. The third char should be 'X', 'r', 's', 't', 'w' or 'x' or 'T'.");
            return false;
        }
        true
    }

    /// The executor doesn't check if the user has the right permissions for this file.
    fn execute_chmod_for_flagged(mode_str: &str, flagged: &Flagged) -> Result<()> {
        let flagged: Vec<_> = flagged
            .content
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let flagged = flagged.join(" ");
        let chmod_args: &str = &format!("chmod {mode_str} {flagged}");

        let executable = "/usr/bin/sh";
        let args = vec!["-c", chmod_args];
        execute_without_output(executable, &args)?;
        Ok(())
    }
}

trait AsOctal<T> {
    /// Converts itself to an octal if possible, 0 otherwise.
    fn as_octal(&self) -> T;
}

impl AsOctal<u32> for str {
    fn as_octal(&self) -> u32 {
        u32::from_str_radix(self, 8).unwrap_or_default()
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
pub fn parse_input_permission(mode_str: &str) -> (Arc<str>, IsValid) {
    log_info!("parse_input_permission: {mode_str}");
    if mode_str.chars().any(|c| c.is_alphabetic()) {
        (Arc::from(""), true)
    } else if mode_str.chars().all(|c| c.is_digit(8)) {
        (permission_mode_to_str(mode_str.as_octal()), true)
    } else {
        (Arc::from("Unreadable mode"), false)
    }
}

struct ModeParser(Mode);

impl ModeParser {
    const fn numeric(&self) -> Mode {
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

    /// Convert a string of 9 chars a numeric mode.
    /// It will only accept basic strings like "rw.r...." or "rw-r----".
    /// Special chars (s, S, t, T) are recognized correctly.
    ///
    /// If `mode_str` isn't 9 chars long, it's rejected.
    /// mode is set to 0.
    /// We simply read the chars and add :
    ///     - Reject if the position isn't possible (S can't be the last char (index=8), it should be in .-xtT)
    ///         This step is just for logging, we could simply add u32::MAX and let the last step do the rejection.
    ///     - Add the corresponding value to the mode.
    fn from_alphabetic(mode_str: &str) -> Option<Self> {
        // rwxrwxrwx
        if mode_str.len() != 9 {
            return None;
        }

        let mut mode = 0;
        for (index, current_char) in mode_str.chars().enumerate() {
            let Some(increment) = Self::evaluate_index_char(index, current_char) else {
                log_info!("Invalid char in permissions '{current_char}' at position {index}");
                return None;
            };
            mode += increment;
        }
        if Self::is_valid_permissions(mode) {
            return Some(Self(mode));
        }

        None
    }

    /// Since every symbol has a value according to its position, we simply associate it.
    /// It should be impossible to have an invalid char
    fn evaluate_index_char(index: usize, current_char: char) -> Option<u32> {
        match current_char {
            '-' | '.' => Some(0o000),

            'r' if index == 0 => Some(0o0400),
            'w' if index == 1 => Some(0o0200),
            'x' if index == 2 => Some(0o0100),
            'S' if index == 2 => Some(0o4000),
            's' if index == 2 => Some(0o4100),

            'r' if index == 3 => Some(0o0040),
            'w' if index == 4 => Some(0o0020),
            'x' if index == 5 => Some(0o0010),
            'S' if index == 5 => Some(0o2000),
            's' if index == 5 => Some(0o2010),

            'r' if index == 6 => Some(0o0004),
            'w' if index == 7 => Some(0o0002),
            'x' if index == 8 => Some(0o0001),
            'T' if index == 8 => Some(0o1000),
            't' if index == 8 => Some(0o1001),

            _ => None,
        }
    }

    const fn is_valid_permissions(mode: Mode) -> bool {
        mode <= MAX_SPECIAL_MODE
    }
}
