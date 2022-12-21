use std::path::{Path, PathBuf};

use crate::impl_selectable_content;

#[derive(Clone, Debug, Default)]
pub struct Flagged {
    pub content: Vec<PathBuf>,
    pub index: usize,
}

impl Flagged {
    pub fn push(&mut self, path: PathBuf) {
        if self.content.contains(&path) {
            return;
        }
        self.content.push(path);
        self.content.sort()
    }

    pub fn remove(&mut self, path: PathBuf) {
        if let Some(index) = self.content.iter().position(|x| *x == path) {
            self.content.remove(index);
        }
    }

    pub fn toggle(&mut self, path: &Path) {
        if let Some(index) = self.content.iter().position(|x| *x == path) {
            self.content.remove(index);
        } else {
            self.push(path.to_path_buf());
        };
    }

    pub fn clear(&mut self) {
        self.content.clear()
    }

    pub fn contains(&self, path: &PathBuf) -> bool {
        self.content.contains(path)
    }

    pub fn filtered(&self, current_path: &Path) -> Vec<&Path> {
        self.content
            .iter()
            .filter(|p| p.starts_with(current_path))
            .map(|p| p.as_path())
            .collect()
    }
}

impl_selectable_content!(PathBuf, Flagged);
