use std::ffi::OsStr;
use std::path::PathBuf;

use crate::impl_content;
use crate::impl_selectable;
use crate::modes::ContentWindow;

#[derive(Debug)]
pub struct Fuzzy {
    pub content: Vec<PathBuf>,
    pub index: usize,
    pub window: ContentWindow,
}

impl Fuzzy {
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
    }

    pub fn select_last(&mut self) {
        self.index = self.content.len().checked_sub(1).unwrap_or_default();
    }

    /// Set the index to the minimum of given index and the maximum possible index (len - 1)
    pub fn select_index(&mut self, index: usize) {
        self.index = index.min(self.content.len().checked_sub(1).unwrap_or_default())
    }

    pub fn page_down(&mut self) {
        for _ in 0..10 {
            self.select_next()
        }
    }

    pub fn page_up(&mut self) {
        for _ in 0..10 {
            self.select_prev()
        }
    }

    pub fn push(&mut self, path: PathBuf) {
        self.content.push(path);
        self.reset_window();
    }

    pub fn reset_window(&mut self) {
        self.window.reset(self.content.len())
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
        self.index.checked_sub(0).unwrap_or_default();
        self.reset_window();
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
}

impl_content!(PathBuf, Fuzzy);
impl_selectable!(Fuzzy);