use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::constant_strings_paths::{CONFIG_FOLDER, HARDCODED_SHORTCUTS};
use crate::git::git_root;
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
    non_mount_size: usize,
}

impl Default for Shortcut {
    fn default() -> Self {
        Self::new()
    }
}

impl Shortcut {
    /// Creates the hardcoded shortcuts
    /// Add the config folder and the git root
    ///(no mount point yet).
    pub fn new() -> Self {
        let mut shortcuts = Self::hardcoded_shortcuts();
        shortcuts = Self::with_home_path(shortcuts);
        shortcuts = Self::with_config_folder(shortcuts);
        shortcuts = Self::with_git_root(shortcuts);
        let non_mount_size = shortcuts.len();
        Self {
            content: shortcuts,
            index: 0,
            non_mount_size,
        }
    }

    fn hardcoded_shortcuts() -> Vec<PathBuf> {
        HARDCODED_SHORTCUTS
            .iter()
            .map(|s| PathBuf::from_str(s).unwrap())
            .collect()
    }

    /// Insert a shortcut to home directory of the current user.
    fn with_home_path(mut shortcuts: Vec<PathBuf>) -> Vec<PathBuf> {
        if let Ok(home_path) = PathBuf::from_str(shellexpand::tilde("~").borrow()) {
            shortcuts.push(home_path);
        }
        shortcuts
    }

    /// Insert a shortcut to config file directory of the current user.
    fn with_config_folder(mut shortcuts: Vec<PathBuf>) -> Vec<PathBuf> {
        if let Ok(config_folder) = PathBuf::from_str(shellexpand::tilde(CONFIG_FOLDER).borrow()) {
            shortcuts.push(config_folder);
        }
        shortcuts
    }

    fn git_root_or_cwd() -> PathBuf {
        if let Ok(git_root) = git_root() {
            PathBuf::from(git_root)
        } else {
            std::env::current_dir().unwrap()
        }
    }

    fn with_git_root(mut shortcuts: Vec<PathBuf>) -> Vec<PathBuf> {
        shortcuts.push(Self::git_root_or_cwd());
        shortcuts
    }

    pub fn update_git_root(&mut self) {
        self.content[self.non_mount_size - 1] = Self::git_root_or_cwd();
    }

    /// Update the shortcuts with the mount points.
    pub fn extend_with_mount_points(&mut self, mount_points: &[&Path]) {
        self.content
            .extend(mount_points.iter().map(|p| p.to_path_buf()));
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh(&mut self, mount_points: &[&Path]) {
        self.content.truncate(self.non_mount_size);
        self.extend_with_mount_points(mount_points)
    }
}

impl_selectable_content!(PathBuf, Shortcut);
