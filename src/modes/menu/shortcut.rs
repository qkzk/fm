use std::borrow::Borrow;
// use std::cmp::min;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::common::{
    current_uid, path_to_config_folder, tilde, HARDCODED_SHORTCUTS, TRASH_FOLDER_FILES,
};
// use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::git_root;
use crate::{impl_content, impl_draw_menu_with_char, impl_selectable, log_info};

/// Holds the hardcoded and mountpoints shortcuts the user can jump to.
/// Also know which shortcut is currently selected by the user.
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// The path to the shortcuts. It's a vector since we can't know how much
    /// mount points are defined.
    pub content: Vec<PathBuf>,
    /// The currently selected shortcut
    pub index: usize,
    start_folder: PathBuf,
}

impl Shortcut {
    /// Creates empty shortcuts.
    /// content won't be initiated before the first opening of the menu.
    #[must_use]
    pub fn empty(start_folder: &Path) -> Self {
        let content = vec![];
        Self {
            content,
            index: 0,
            start_folder: start_folder.to_owned(),
        }
    }

    fn build_content(start_folder: &Path) -> Vec<PathBuf> {
        let mut content = Self::hardcoded_shortcuts();
        Self::push_home_path(&mut content);
        Self::push_trash_folder(&mut content);
        Self::push_config_folder(&mut content);
        Self::push_start_folder(&mut content, start_folder);
        Self::push_git_root(&mut content);
        content
    }

    pub fn update(&mut self) {
        self.content = Self::build_content(&self.start_folder)
    }

    fn hardcoded_shortcuts() -> Vec<PathBuf> {
        HARDCODED_SHORTCUTS.iter().map(PathBuf::from).collect()
    }

    /// Insert a shortcut to home directory of the current user.
    fn push_home_path(shortcuts: &mut Vec<PathBuf>) {
        if let Ok(home_path) = PathBuf::from_str(tilde("~").borrow()) {
            shortcuts.push(home_path);
        }
    }

    /// Insert a shortcut to trash directory of the current user.
    fn push_trash_folder(shortcuts: &mut Vec<PathBuf>) {
        if let Ok(trash_path) = PathBuf::from_str(tilde(TRASH_FOLDER_FILES).borrow()) {
            if trash_path.exists() {
                shortcuts.push(trash_path);
            }
        }
    }

    /// Insert a shortcut to config file directory of the current user.
    fn push_config_folder(shortcuts: &mut Vec<PathBuf>) {
        if let Ok(config_folder) = path_to_config_folder() {
            shortcuts.push(config_folder);
        }
    }

    fn push_start_folder(shortcuts: &mut Vec<PathBuf>, start_folder: &Path) {
        shortcuts.push(start_folder.to_owned());
    }

    fn git_root_or_cwd() -> PathBuf {
        git_root().map_or_else(
            |_| std::env::current_dir().unwrap_or_default(),
            PathBuf::from,
        )
    }

    fn push_git_root(shortcuts: &mut Vec<PathBuf>) {
        shortcuts.push(Self::git_root_or_cwd());
    }

    fn clear_doublons(&mut self) {
        self.content.sort_unstable();
        self.content.dedup();
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

    /// Refresh the shortcuts.
    /// Lazy loading.
    /// As long as this method isn't called, content won't be populated.
    pub fn refresh(
        &mut self,
        mount_points: &[&Path],
        left_path: &std::path::Path,
        right_path: &std::path::Path,
    ) {
        self.content = Self::build_content(&self.start_folder);
        self.content.push(left_path.to_owned());
        self.content.push(right_path.to_owned());
        self.extend_with_mount_points(mount_points);
    }
}

impl_selectable!(Shortcut);
impl_content!(Shortcut, PathBuf);
impl_draw_menu_with_char!(Shortcut, PathBuf);
