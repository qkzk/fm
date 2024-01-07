use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::impl_content;
use crate::impl_selectable;
use crate::modes::ContentWindow;

#[derive(Clone, Debug)]
pub struct Flagged {
    /// Contains the different flagged files.
    /// It's basically a `Set` (of whatever kind) and insertion would be faster
    /// using a set.
    /// Iteration is faster with a vector and we need a vector to use the common trait
    /// `SelectableContent` which can be implemented with a macro.
    /// We use binary search in every non standard method (insertion, removal, search).
    pub content: Vec<PathBuf>,
    /// The index of the selected file. Used to jump.
    pub index: usize,
    pub window: ContentWindow,
}

impl Flagged {
    pub fn new(content: Vec<PathBuf>, terminal_height: usize) -> Self {
        Self {
            window: ContentWindow::new(content.len(), terminal_height),
            content,
            index: 0,
        }
    }

    pub fn select_next(&mut self) {
        self.next();
        self.window.scroll_down_one(self.index);
    }

    pub fn select_prev(&mut self) {
        self.prev();
        self.window.scroll_up_one(self.index);
    }

    pub fn select_first(&mut self) {
        self.index = 0;
        self.window.scroll_to(0);
    }

    pub fn select_last(&mut self) {
        self.index = self.content.len().checked_sub(1).unwrap_or_default();
        self.window.scroll_to(self.index);
    }

    /// Set the index to the minimum of given index and the maximum possible index (len - 1)
    pub fn select_index(&mut self, index: usize) {
        self.index = index.min(self.content.len().checked_sub(1).unwrap_or_default());
        self.window.scroll_to(self.index);
    }

    pub fn select_row(&mut self, row: u16) {
        let index = row.checked_sub(4).unwrap_or_default() as usize + self.window.top;
        self.select_index(index);
    }

    pub fn page_down(&mut self) {
        for _ in 0..10 {
            if self.index + 1 == self.content.len() {
                break;
            }
            self.select_next();
        }
    }

    pub fn page_up(&mut self) {
        for _ in 0..10 {
            if self.index == 0 {
                break;
            }
            self.select_prev();
        }
    }

    pub fn reset_window(&mut self) {
        self.window.reset(self.content.len())
    }

    pub fn set_height(&mut self, height: usize) {
        self.window.set_height(height);
    }

    pub fn update(&mut self, content: Vec<PathBuf>) {
        self.content = content;
        self.reset_window();
        self.index = 0;
    }

    pub fn clear(&mut self) {
        self.content = vec![];
        self.reset_window();
        self.index = 0;
    }

    pub fn remove_selected(&mut self) {
        self.content.remove(self.index);
        if self.index > 0 {
            self.index -= 1;
        }
        self.reset_window();
    }

    pub fn replace_selected(&mut self, new_path: PathBuf) {
        if self.content.is_empty() {
            return;
        }
        self.content[self.index] = new_path;
    }

    pub fn filenames_containing(&self, input_string: &str) -> Vec<String> {
        let to_filename: fn(&PathBuf) -> Option<&OsStr> = |path| path.file_name();
        let to_str: fn(&OsStr) -> Option<&str> = |filename| filename.to_str();
        self.content
            .iter()
            .filter_map(to_filename)
            .filter_map(to_str)
            .filter(|&p| p.contains(input_string))
            .map(|p| p.to_owned())
            .collect()
    }

    /// Push a new path into the content.
    /// We maintain the content sorted and it's used to make `contains` faster.
    pub fn push(&mut self, path: PathBuf) {
        match self.content.binary_search(&path) {
            Ok(_) => (),
            Err(pos) => {
                self.content.insert(pos, path);
                self.reset_window()
            }
        }
    }

    /// Toggle the flagged status of a path.
    /// Remove the path from the content if it's flagged, flag it if it's not.
    /// The implantation assumes the content to be sorted.
    pub fn toggle(&mut self, path: &Path) {
        let path_buf = path.to_path_buf();
        match self.content.binary_search(&path_buf) {
            Ok(pos) => {
                self.content.remove(pos);
            }
            Err(pos) => {
                self.content.insert(pos, path_buf);
            }
        }
        self.reset_window();
    }

    /// True if the `path` is flagged.
    /// Since we maintain the content sorted, we can use a binary search and
    /// compensate a little bit with using a vector instead of a set.
    #[inline]
    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.content.binary_search(&path.to_path_buf()).is_ok()
    }

    pub fn replace(&mut self, old_path: &Path, new_path: &Path) {
        let Ok(index) = self.content.binary_search(&old_path.to_path_buf()) else {
            return;
        };
        self.content[index] = new_path.to_owned();
    }

    /// Returns a vector of path which are present in the current directory.
    #[inline]
    #[must_use]
    pub fn in_dir(&self, dir: &Path) -> Vec<PathBuf> {
        self.content
            .iter()
            .filter(|p| p.starts_with(dir))
            .map(|p| p.to_owned())
            .collect()
    }

    /// Basic search in flagged mode.
    /// Select the first path whose last component (aka its filename) contains the searched pattern.
    pub fn search(&mut self, searched: &str) {
        let Some(position) = self.content.iter().position(|path| {
            path.components()
                .last()
                .unwrap()
                .as_os_str()
                .to_string_lossy()
                .contains(searched)
        }) else {
            return;
        };
        self.select_index(position);
    }
}

// impl_selectable_content!(PathBuf, Flagged);
impl_selectable!(Flagged);
impl_content!(PathBuf, Flagged);
