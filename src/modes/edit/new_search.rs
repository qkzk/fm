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
                next_index = index;
                if !found {
                    self.index = self.paths.len();
                    found = true;
                }
                self.paths.push(file.path.to_path_buf());
            }
        }
        for (index, file) in tab.directory.enumerate().take(current_index) {
            if self.regex.is_match(&file.filename) {
                next_index = index;
                if !found {
                    self.index = self.paths.len();
                    found = true;
                }
                self.paths.push(file.path.to_path_buf());
            }
        }

        tab.go_to_index(next_index);
    }

    pub fn directory_search_next(&mut self, tab: &Tab) -> Option<std::path::PathBuf> {
        if !self.paths.is_empty() {
            let index = (self.index + 1) % self.paths.len();
            return Some(self.paths[index].to_owned());
        }
        let current_index = tab.directory.index;
        let mut found = false;
        for file in tab.directory.iter().skip(current_index) {
            if self.regex.is_match(&file.filename) {
                if !found {
                    self.index = self.paths.len();
                    found = true;
                }
                self.paths.push(file.path.to_path_buf());
            }
        }
        for file in tab.directory.iter().take(current_index) {
            if self.regex.is_match(&file.filename) {
                if !found {
                    self.index = self.paths.len();
                    found = true;
                }
                self.paths.push(file.path.to_path_buf());
            }
        }
        if found {
            Some(self.paths[self.index].to_owned())
        } else {
            None
        }
    }

    pub fn tree(&self, tree: &mut Tree) {
        let path = self.tree_find_next_path(tree).to_owned();
        tree.go(To::Path(&path));
    }

    fn tree_find_next_path<'a>(&self, tree: &'a mut Tree) -> &'a std::path::Path {
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
                return &line.path;
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
                return &line.path;
            }
        }

        tree.selected_path()
    }

    pub fn flagged(&self, flagged: &mut Flagged) {
        let position = if let Some(pos) =
            flagged
                .content
                .iter()
                .skip(flagged.index + 1)
                .position(|path| {
                    self.regex
                        .is_match(&path.file_name().unwrap().to_string_lossy())
                }) {
            pos + flagged.index + 1
        } else if let Some(pos) = flagged
            .content
            .iter()
            .take(flagged.index + 1)
            .position(|path| {
                self.regex
                    .is_match(&path.file_name().unwrap().to_string_lossy())
            })
        {
            pos
        } else {
            return;
        };

        flagged.select_index(position);
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
