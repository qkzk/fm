use anyhow::Result;

use crate::{
    app::{Status, Tab},
    modes::{Content, Display, Flagged, Go, To, ToPath, Tree},
};

pub struct Search {
    pub regex: regex::Regex,
    pub paths: Vec<std::path::PathBuf>,
    pub index: usize,
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

    pub fn reset_paths(&mut self) {
        self.paths = vec![];
        self.index = 0;
    }

    pub fn position_str(&self) -> String {
        if self.paths.is_empty() {
            "0 / 0".to_owned()
        } else {
            format!(
                "{ind} / {len}",
                ind = self.index + 1,
                len = self.paths.len()
            )
        }
    }

    pub fn leave(&mut self, status: &mut Status) -> Result<()> {
        match status.current_tab().display_mode {
            Display::Tree => {
                self.tree(&mut status.current_tab_mut().tree);
            }
            Display::Directory => {
                self.directory(status.current_tab_mut());
            }
            Display::Flagged => self.flagged(&mut status.menu.flagged),
            _ => (),
        };
        status.update_second_pane_for_preview()
    }

    /// Search in current directory for an file whose name contains `searched_name`,
    /// from a starting position `next_index`.
    /// We search forward from that position and start again from top if nothing is found.
    /// We move the selection to the first matching file.
    pub fn directory(&mut self, tab: &mut Tab) {
        let current_index = tab.directory.index;
        let mut next_index = current_index;
        let mut found = false;
        for (index, file) in tab.directory.enumerate().skip(current_index) {
            if self.regex.is_match(&file.filename) {
                if !found {
                    next_index = index;
                    self.index = self.paths.len();
                    found = true;
                }
                self.paths.push(file.path.to_path_buf());
            }
        }
        for (index, file) in tab.directory.enumerate().take(current_index) {
            if self.regex.is_match(&file.filename) {
                if !found {
                    next_index = index;
                    self.index = self.paths.len();
                    found = true;
                }
                self.paths.push(file.path.to_path_buf());
            }
        }

        tab.go_to_index(next_index);
    }

    pub fn select_next(&mut self) -> Option<std::path::PathBuf> {
        if !self.paths.is_empty() {
            self.index = (self.index + 1) % self.paths.len();
            return Some(self.paths[self.index].to_owned());
        }
        None
    }

    pub fn directory_search_next(
        &self,
        tab: &Tab,
    ) -> (
        Vec<std::path::PathBuf>,
        Option<usize>,
        Option<std::path::PathBuf>,
    ) {
        let current_index = tab.directory.index;
        let mut paths = vec![];
        let mut found = false;
        let mut index = None;
        let mut found_path = None;
        for file in tab.directory.iter().skip(current_index) {
            if self.regex.is_match(&file.filename) {
                if !found {
                    index = Some(self.paths.len());
                    found = true;
                    found_path = Some(file.path.to_path_buf());
                }
                paths.push(file.path.to_path_buf());
            }
        }
        for file in tab.directory.iter().take(current_index) {
            if self.regex.is_match(&file.filename) {
                if !found {
                    index = Some(self.paths.len());
                    found = true;
                    found_path = Some(file.path.to_path_buf());
                }
                paths.push(file.path.to_path_buf());
            }
        }
        (paths, index, found_path)
    }

    pub fn set_index_paths(&mut self, index: usize, paths: Vec<std::path::PathBuf>) {
        self.paths = paths;
        self.index = index;
    }

    pub fn tree(&mut self, tree: &mut Tree) {
        if let Some(path) = self.tree_find_next_path(tree).to_owned() {
            tree.go(To::Path(&path));
        }
    }

    fn tree_find_next_path<'a>(&mut self, tree: &'a mut Tree) -> Option<std::path::PathBuf> {
        if !self.paths.is_empty() {
            self.index = (self.index + 1) % self.paths.len();
            return Some(self.paths[self.index].to_owned());
        }
        let mut found_path = None;
        let mut found = false;
        for line in tree
            .displayable()
            .lines()
            .iter()
            .skip(tree.displayable().index() + 1)
        {
            let Some(filename) = line.path.file_name() else {
                continue;
            };
            if self.regex.is_match(&filename.to_string_lossy()) {
                self.paths.push(line.path.to_path_buf());
                if !found {
                    self.index = self.paths.len();
                    found_path = Some(line.path.to_path_buf());
                    found = true;
                }
            }
        }
        for line in tree
            .displayable()
            .lines()
            .iter()
            .take(tree.displayable().index())
        {
            let Some(filename) = line.path.file_name() else {
                continue;
            };
            if self.regex.is_match(&filename.to_string_lossy()) {
                if !found {
                    self.paths.push(line.path.to_path_buf());
                    self.index = self.paths.len();
                    found_path = Some(line.path.to_path_buf());
                    found = true;
                }
            }
        }

        found_path
    }

    pub fn flagged(&mut self, flagged: &mut Flagged) {
        if !self.paths.is_empty() {
            self.index = (self.index + 1) % self.paths.len();
            flagged.select_path(&self.paths[self.index]);
        }

        if let Some(path) = self.find_in_flagged(flagged) {
            flagged.select_path(&path);
        }
    }

    fn find_in_flagged(&mut self, flagged: &Flagged) -> Option<std::path::PathBuf> {
        let mut found = false;
        let mut found_path = None;

        for path in flagged.content.iter().skip(flagged.index + 1) {
            if self
                .regex
                .is_match(&path.file_name().unwrap().to_string_lossy())
            {
                if !found {
                    found = true;
                    found_path = Some(path.to_path_buf());
                    self.index = self.paths.len();
                }
                self.paths.push(path.to_path_buf());
            }
        }
        for path in flagged.content.iter().take(flagged.index + 1) {
            if self
                .regex
                .is_match(&path.file_name().unwrap().to_string_lossy())
            {
                if !found {
                    found = true;
                    found_path = Some(path.to_path_buf());
                    self.index = self.paths.len();
                }
                self.paths.push(path.to_path_buf());
            }
        }
        found_path
    }

    pub fn complete_flagged(&self, flagged: &Flagged) -> Vec<String> {
        self.filtered_paths(flagged.content())
            .iter()
            .filter_map(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .collect()
    }

    pub fn complete_tree(&self, tree: &Tree) -> Vec<String> {
        self.filtered_paths(tree.displayable().lines())
            .iter()
            .filter_map(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .collect()
    }

    pub fn complete_directory(&self, tab: &Tab) -> Vec<String> {
        self.filtered_paths(tab.directory.content())
            .iter()
            .filter_map(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .collect()
    }

    pub fn filtered_paths(&self, content: &Vec<impl ToPath>) -> Vec<std::path::PathBuf> {
        content
            .iter()
            .map(|elt| elt.to_path())
            .filter(|p| {
                self.regex
                    .is_match(p.file_name().unwrap_or_default().to_string_lossy().as_ref())
            })
            .map(|p| p.to_owned())
            .collect()
    }
}
