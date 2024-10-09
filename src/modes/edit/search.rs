use std::path::PathBuf;

use anyhow::Result;

use crate::app::Tab;
use crate::modes::{Display, FileInfo, Go, IndexToIndex, To, ToPath, Tree};

#[derive(Clone)]
pub struct Search {
    pub regex: regex::Regex,
    pub paths: Vec<PathBuf>,
    pub index: usize,
}

impl std::fmt::Display for Search {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.is_empty() {
            write!(f, "")
        } else {
            write!(
                f,
                "Searched: {regex} - {pos} / {len}",
                regex = self.regex,
                pos = self.index + 1 - self.paths.is_empty() as usize,
                len = self.paths.len()
            )
        }
    }
}

impl Search {
    pub fn empty() -> Self {
        Self {
            regex: regex::Regex::new("").unwrap(),
            paths: vec![],
            index: 0,
        }
    }

    pub fn new(searched: &str) -> Result<Self> {
        Ok(Self {
            regex: regex::Regex::new(searched)?,
            paths: vec![],
            index: 0,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.regex.to_string().is_empty()
    }

    pub fn reset_paths(&mut self) {
        self.paths = vec![];
        self.index = 0;
    }

    pub fn select_next(&mut self) -> Option<PathBuf> {
        if !self.paths.is_empty() && !self.regex.to_string().is_empty() {
            self.index = (self.index + 1) % self.paths.len();
            return Some(self.paths[self.index].to_owned());
        }
        None
    }

    pub fn execute_search(&mut self, tab: &mut Tab) -> Result<()> {
        match tab.display_mode {
            Display::Tree => {
                self.tree(&mut tab.tree);
            }
            Display::Directory => {
                self.directory(tab);
            }
            _ => (),
        };
        Ok(())
    }

    /// Search in current directory for an file whose name contains `searched_name`,
    /// from a starting position `next_index`.
    /// We search forward from that position and start again from top if nothing is found.
    /// We move the selection to the first matching file.
    #[inline]
    fn directory(&mut self, tab: &mut Tab) {
        let current_index = tab.directory.index;
        let mut next_index = current_index;
        let mut found = false;
        for (index, file) in tab.directory.enumerate().skip(current_index) {
            (next_index, found) = self.set_found(index, file, found, next_index);
        }
        for (index, file) in tab.directory.enumerate().take(current_index) {
            (next_index, found) = self.set_found(index, file, found, next_index);
        }

        tab.go_to_index(next_index);
    }

    #[inline]
    fn set_found(
        &mut self,
        index: usize,
        file: &FileInfo,
        mut found: bool,
        mut next_index: usize,
    ) -> (usize, bool) {
        if self.regex.is_match(&file.filename) {
            if !found {
                next_index = index;
                self.index = self.paths.len();
                found = true;
            }
            self.paths.push(file.path.to_path_buf());
        }
        (next_index, found)
    }

    pub fn directory_search_next(
        &self,
        tab: &Tab,
    ) -> (Vec<PathBuf>, Option<usize>, Option<PathBuf>) {
        let mut paths = vec![];
        let mut next_index = None;
        let mut next_path = None;

        for file in tab.directory.index_to_index() {
            if self.regex.is_match(&file.filename) {
                if next_index.is_none() {
                    (next_index, next_path) = self.found_first_match(file)
                }
                paths.push(file.path.to_path_buf());
            }
        }
        (paths, next_index, next_path)
    }

    fn found_first_match(&self, file: &FileInfo) -> (Option<usize>, Option<PathBuf>) {
        (Some(self.paths.len()), Some(file.path.to_path_buf()))
    }

    pub fn set_index_paths(&mut self, index: usize, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.index = index;
    }

    pub fn tree(&mut self, tree: &mut Tree) {
        if let Some(path) = &self.tree_find_next_path(tree) {
            tree.go(To::Path(path));
        }
    }

    fn tree_find_next_path(&mut self, tree: &mut Tree) -> Option<PathBuf> {
        if let Some(path) = self.select_next() {
            return Some(path);
        } else {
            self.reset_paths()
        }
        let mut next_path = None;
        for line in tree.index_to_index() {
            let Some(filename) = line.path.file_name() else {
                continue;
            };
            if self.regex.is_match(&filename.to_string_lossy()) {
                let match_path = line.path.to_path_buf();
                if next_path.is_none() {
                    self.index = self.paths.len();
                    next_path = Some(match_path.clone());
                }
                self.paths.push(match_path);
            }
        }
        next_path
    }

    #[inline]
    pub fn complete(&self, content: &[impl ToPath]) -> Vec<String> {
        content
            .iter()
            .filter_map(|e| e.to_path().file_name())
            .map(|s| s.to_string_lossy().to_string())
            .filter(|p| self.regex.is_match(p))
            .collect()
    }
}
