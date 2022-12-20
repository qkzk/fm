use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::constant_strings_paths::HARDCODED_SHORTCUTS;

/// Holds the hardcoded and mountpoints shortcuts the user can jump to.
/// Also know which shortcut is currently selected by the user.
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// The path to the shortcuts. It's a vector since we can't know how much
    /// mount points are defined.
    pub shortcuts: Vec<PathBuf>,
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
            shortcuts,
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
        self.shortcuts
            .extend(mount_points.iter().map(|p| p.to_path_buf()));
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh(&mut self, mount_points: &[&Path]) {
        self.shortcuts.truncate(HARDCODED_SHORTCUTS.len() + 1);
        self.extend_with_mount_points(mount_points)
    }

    fn is_empty(&self) -> bool {
        self.shortcuts.is_empty()
    }

    fn len(&self) -> usize {
        self.shortcuts.len()
    }

    /// Select the next shortcut.
    pub fn next(&mut self) {
        if self.is_empty() {
            self.index = 0;
        } else {
            self.index = (self.index + 1) % self.len()
        }
    }

    /// Select the previous shortcut.
    pub fn prev(&mut self) {
        if self.is_empty() {
            self.index = 0
        } else if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.len() - 1
        }
    }

    /// Returns the pathbuf of the currently selected shortcut.
    pub fn selected(&self) -> PathBuf {
        self.shortcuts[self.index].clone()
    }
}
