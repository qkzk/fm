use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use ratatui::style::Color;
use ratatui::{layout::Rect, Frame};
use std::cmp::min;

use crate::colored_skip_take;
use crate::common::{
    current_uid, path_to_config_folder, tilde, HARDCODED_SHORTCUTS, TRASH_FOLDER_FILES,
};
use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::{color_to_style, git_root, Canvas, CowStr, DrawMenu};
use crate::modes::ContentWindow;
use crate::{impl_content, impl_selectable, log_info};

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
        Self::push_home_path(&mut shortcuts);
        Self::push_trash_folder(&mut shortcuts);
        Self::push_config_folder(&mut shortcuts);
        Self::push_start_folder(&mut shortcuts, start_folder);
        Self::push_git_root(&mut shortcuts);
        let non_mount_size = shortcuts.len();
        Self {
            content: shortcuts,
            index: 0,
            non_mount_size,
        }
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
        self.content.dedup();
        dedup_slow(&mut self.content);
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
fn dedup_slow<T>(elems: &mut Vec<T>)
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
    for j in to_remove.iter().rev() {
        elems.remove(*j);
    }
}
impl_selectable!(Shortcut);
impl_content!(PathBuf, Shortcut);

impl DrawMenu<PathBuf> for Shortcut {
    fn draw_menu(&self, f: &mut Frame, rect: &Rect, window: &ContentWindow)
    where
        Self: Content<PathBuf>,
    {
        let content = self.content();
        for (letter, (index, path, style)) in std::iter::zip(
            ('a'..='z').cycle().skip(window.top),
            colored_skip_take!(content, window),
        ) {
            let style = self.style(index, &style);
            let row = index as u16 + ContentWindow::WINDOW_MARGIN_TOP_U16 + 1 - window.top as u16;
            if row + 2 > rect.height {
                return;
            }
            rect.print_with_style(f, row, 2, &format!("{letter} "), style);
            rect.print_with_style(f, row, 4, &path.cow_str(), self.style(index, &style));
        }
    }
}
