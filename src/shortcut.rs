use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::constant_strings_paths::HARDCODED_SHORTCUTS;
use crate::impl_selectable_content;

/// Holds the hardcoded and mountpoints shortcuts the user can jump to.
/// Also know which shortcut is currently selected by the user.
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// The path to the shortcuts. It's a vector since we can't know how much
    /// mount points are defined.
    pub content: Vec<PathBuf>,
    /// The currently selected shortcut
    pub index: usize,
}

impl Default for Shortcut {
    fn default() -> Self {
        Self::new()
    }
}

impl Shortcut {
    /// Creates the hardcoded shortcuts (no mount point yet).
    pub fn new() -> Self {
        let mut shortcuts = Self::hardcoded_shortcuts();
        shortcuts = Self::with_home_path(shortcuts);
        Self {
            content: shortcuts,
            index: 0,
        }
    }

    fn hardcoded_shortcuts() -> Vec<PathBuf> {
        HARDCODED_SHORTCUTS
            .iter()
            .map(|s| PathBuf::from_str(s).unwrap())
            .collect()
    }

    /// Insert a shortcut to home directory of the current user.
    pub fn with_home_path(mut shortcuts: Vec<PathBuf>) -> Vec<PathBuf> {
        if let Ok(pb) = PathBuf::from_str(shellexpand::tilde("~").borrow()) {
            shortcuts.push(pb);
        }
        shortcuts
    }

    /// Update the shortcuts with the mount points.
    pub fn extend_with_mount_points(&mut self, mount_points: &[&Path]) {
        self.content
            .extend(mount_points.iter().map(|p| p.to_path_buf()));
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh(&mut self, mount_points: &[&Path]) {
        self.content.truncate(HARDCODED_SHORTCUTS.len() + 1);
        self.extend_with_mount_points(mount_points)
    }
}

impl_selectable_content!(PathBuf, Shortcut);
