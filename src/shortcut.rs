use std::borrow::Borrow;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct Shortcut {
    pub shortcuts: Vec<PathBuf>,
    pub index: usize,
}

impl Default for Shortcut {
    fn default() -> Self {
        Self::new()
    }
}

impl Shortcut {
    pub fn new() -> Self {
        let shortcuts = Self::reset_shortcuts();
        Self {
            shortcuts,
            index: 0,
        }
    }

    fn reset_shortcuts() -> Vec<PathBuf> {
        vec![
            PathBuf::from_str("/").unwrap(),
            PathBuf::from_str("/dev").unwrap(),
            PathBuf::from_str("/etc").unwrap(),
            PathBuf::from_str("/media").unwrap(),
            PathBuf::from_str("/mnt").unwrap(),
            PathBuf::from_str("/opt").unwrap(),
            PathBuf::from_str("/run/media").unwrap(),
            PathBuf::from_str("/tmp").unwrap(),
            PathBuf::from_str("/usr").unwrap(),
            PathBuf::from_str(shellexpand::tilde("~").borrow()).unwrap(),
        ]
    }

    pub fn update_mount_points(&mut self, mount_points: Vec<&Path>) {
        let mut shortcuts = Self::reset_shortcuts();
        shortcuts.extend(mount_points.iter().map(|p| p.to_path_buf()));
        self.shortcuts = shortcuts;
    }

    pub fn is_empty(&self) -> bool {
        self.shortcuts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.shortcuts.len()
    }

    pub fn next(&mut self) {
        if self.is_empty() {
            self.index = 0;
        } else {
            self.index = (self.index + 1) % self.len()
        }
    }

    pub fn prev(&mut self) {
        if self.is_empty() {
            self.index = 0
        } else if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.len() - 1
        }
    }

    pub fn selected(&self) -> PathBuf {
        self.shortcuts[self.index].clone()
    }
}
