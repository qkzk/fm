use std::path::PathBuf;

use crate::impl_content;
use crate::impl_selectable;
use crate::modes::ContentWindow;
use crate::modes::FileInfo;
use crate::modes::Users;

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

    pub fn page_down(&mut self) {
        for _ in 0..10 {
            self.next()
        }
    }

    pub fn page_up(&mut self) {
        for _ in 0..10 {
            self.prev()
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

    pub fn into_fileinfos(&self, users: &Users) -> Vec<FileInfo> {
        self.content
            .iter()
            .filter_map(|p| FileInfo::new(&p, users).ok())
            .collect()
    }
}

impl_content!(PathBuf, Fuzzy);
impl_selectable!(Fuzzy);
