use std::os::unix::fs::PermissionsExt;

use anyhow::Result;

use crate::log_line;
use crate::modes::Flagged;

type Mode = u32;

pub struct Permissions;

impl Permissions {
    /// Set the permissions of the flagged files according to a given permission.
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
    pub fn set_permissions_of_flagged(mode_str: &str, flagged: &mut Flagged) -> Result<()> {
        let Some(mode) = _Mode::from_str(mode_str) else {
            return Ok(());
        };
        for path in flagged.content.iter() {
            Self::set_permissions(path, mode.octal())?
        }
        flagged.clear();
        log_line!("Changed permissions to {mode_str}");
        Ok(())
    }
}

struct _Mode(Mode);

impl _Mode {
    /// Max valid mode, ie `0o777`.
    const MAX_MODE: Mode = 0o777;

    fn octal(&self) -> Mode {
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

    fn is_valid_permissions(mode: Mode) -> bool {
        0 < mode && mode <= Self::MAX_MODE
    }
}
