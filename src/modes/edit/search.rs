use anyhow::Result;

use crate::{
    app::{Status, Tab},
    modes::{Content, Display, Flagged, Go, To, ToPath, Tree},
};

#[derive(Debug, Default)]
pub struct Search {
    pub regex: Option<regex::Regex>,
}

impl Search {
    pub fn new(searched: &str) -> Result<Self> {
        Ok(Self {
            regex: regex::Regex::new(searched).ok(),
        })
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
    pub fn directory(&self, tab: &mut Tab) {
        let Some(re) = &self.regex else {
            return;
        };
        let current_index = tab.directory.index;
        let next_index = if let Some(index) = Self::directory_from_index(tab, re, current_index) {
            index
        } else if let Some(index) = Self::directory_from_top(tab, re, current_index) {
            index
        } else {
            return;
        };
        tab.go_to_index(next_index);
    }

    pub fn directory_search_next(&self, tab: &Tab) -> Option<usize> {
        let Some(re) = &self.regex else {
            return None;
        };
        let current_index = tab.directory.index;
        if let Some(index) = Self::directory_from_index(tab, re, current_index) {
            Some(index)
        } else if let Some(index) = Self::directory_from_top(tab, re, current_index) {
            Some(index)
        } else {
            None
        }
    }

    /// Search a file by filename from given index, moving down
    fn directory_from_index(tab: &Tab, re: &regex::Regex, current_index: usize) -> Option<usize> {
        for (index, file) in tab.directory.enumerate().skip(current_index) {
            if re.is_match(&file.filename) {
                return Some(index);
            }
        }
        None
    }

    /// Search a file by filename from first line, moving down
    fn directory_from_top(tab: &Tab, re: &regex::Regex, current_index: usize) -> Option<usize> {
        for (index, file) in tab.directory.enumerate().take(current_index) {
            if re.is_match(&file.filename) {
                return Some(index);
            }
        }
        None
    }

    pub fn tree(&self, tree: &mut Tree) {
        let path = self.tree_find_next_path(tree).to_owned();
        tree.go(To::Path(&path));
    }

    fn tree_find_next_path<'a>(&self, tree: &'a mut Tree) -> &'a std::path::Path {
        if let Some(pattern) = &self.regex {
            for line in tree
                .displayable()
                .lines()
                .iter()
                .skip(tree.displayable().index() + 1)
            {
                let Some(filename) = line.path.file_name() else {
                    continue;
                };
                if pattern.is_match(&filename.to_string_lossy()) {
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
                if pattern.is_match(&filename.to_string_lossy()) {
                    return &line.path;
                }
            }
        }
        tree.selected_path()
    }

    pub fn flagged(&self, flagged: &mut Flagged) {
        if let Some(re) = &self.regex {
            let position = if let Some(pos) = flagged
                .content
                .iter()
                .skip(flagged.index + 1)
                .position(|path| re.is_match(&path.file_name().unwrap().to_string_lossy()))
            {
                pos + flagged.index + 1
            } else if let Some(pos) = flagged
                .content
                .iter()
                .take(flagged.index + 1)
                .position(|path| re.is_match(&path.file_name().unwrap().to_string_lossy()))
            {
                pos
            } else {
                return;
            };

            flagged.select_index(position);
        }
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
        if let Some(re) = &self.regex {
            content
                .iter()
                .map(|elt| elt.to_path())
                .filter(|p| {
                    re.is_match(p.file_name().unwrap_or_default().to_string_lossy().as_ref())
                })
                .map(|p| p.to_owned())
                .collect()
        } else {
            vec![]
        }
    }
}
