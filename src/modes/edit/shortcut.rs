use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::common::current_uid;
use crate::common::{CONFIG_FOLDER, HARDCODED_SHORTCUTS};
use crate::impl_content;
use crate::impl_selectable;
use crate::io::git_root;
use crate::log_info;

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

impl Shortcut {
    /// Creates the hardcoded shortcuts
    /// Add the config folder and the git root
    ///(no mount point yet).
    #[must_use]
    pub fn new(start_folder: &Path) -> Self {
        let mut shortcuts = Self::hardcoded_shortcuts();
        shortcuts = Self::with_home_path(shortcuts);
        shortcuts = Self::with_config_folder(shortcuts);
        shortcuts = Self::with_start_folder(shortcuts, start_folder);
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
            .map(PathBuf::from)
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

    fn with_start_folder(mut shortcuts: Vec<PathBuf>, start_folder: &Path) -> Vec<PathBuf> {
        shortcuts.push(start_folder.to_owned());
        shortcuts
    }

    fn git_root_or_cwd() -> PathBuf {
        git_root().map_or_else(
            |_| std::env::current_dir().unwrap_or_default(),
            PathBuf::from,
        )
    }

    fn with_git_root(mut shortcuts: Vec<PathBuf>) -> Vec<PathBuf> {
        shortcuts.push(Self::git_root_or_cwd());
        shortcuts
    }

    fn clear_doublons(&mut self) {
        self.content.dedup();
        self.content = dedup_slow(self.content.clone());
    }

    pub fn update_git_root(&mut self) {
        self.content[self.non_mount_size - 1] = Self::git_root_or_cwd();
    }

    pub fn with_mount_points(mut self, mount_points: &[&Path]) -> Self {
        self.extend_with_mount_points(mount_points);
        self
    }

    /// Update the shortcuts with the mount points.
    fn extend_with_mount_points(&mut self, mount_points: &[&Path]) {
        self.content
            .extend(mount_points.iter().map(|p| p.to_path_buf()));
        self.extend_with_mtp();
        self.clear_doublons();
    }

    /// Update the shortcuts with MTP mount points
    fn extend_with_mtp(&mut self) {
        let Ok(uid) = current_uid() else {
            return;
        };
        let mtp_mount_point = PathBuf::from(format!("/run/user/{uid}/gvfs/"));
        if !mtp_mount_point.exists() || !mtp_mount_point.is_dir() {
            return;
        }

        let mount_points: Vec<PathBuf> = match std::fs::read_dir(&mtp_mount_point) {
            Ok(read_dir) => read_dir
                .filter_map(std::result::Result::ok)
                .filter(|direntry| direntry.path().is_dir())
                .map(|direntry| direntry.path())
                .collect(),
            Err(error) => {
                log_info!(
                    "unreadable gvfs {mtp_mount_point}: {error:?} ",
                    mtp_mount_point = mtp_mount_point.display(),
                );
                return;
            }
        };
        self.content.extend(mount_points);
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh(
        &mut self,
        mount_points: &[&Path],
        left_path: &std::path::Path,
        right_path: &std::path::Path,
    ) {
        self.content.truncate(self.non_mount_size);
        self.content.push(left_path.to_owned());
        self.content.push(right_path.to_owned());
        self.extend_with_mount_points(mount_points);
    }
}

/// Remove duplicates from a vector and returns it.
/// Elements should be `PartialEq`.
/// It removes element than are not consecutives and is very slow.
fn dedup_slow<T>(mut elems: Vec<T>) -> Vec<T>
where
    T: PartialEq,
{
    let mut to_remove = vec![];
    for i in 0..elems.len() {
        for j in (i + 1)..elems.len() {
            if elems[i] == elems[j] {
                to_remove.push(j);
            }
        }
    }
    for i in to_remove.iter().rev() {
        elems.remove(*i);
    }
    elems
}

// impl_selectable_content!(PathBuf, Shortcut);
impl_selectable!(Shortcut);
impl_content!(PathBuf, Shortcut);
