use anyhow::Result;

use crate::{
    app::{Status, Tab},
    modes::{Display, Flagged, Go, To, Tree},
};

#[derive(Clone, Debug, Default)]
pub struct Search {
    pub regex: Option<regex::Regex>,
    index: Option<usize>,
    nb_matches: Option<usize>,
}

impl Search {
    pub fn new(searched: &str) -> Result<Self> {
        Ok(Self {
            regex: regex::Regex::new(searched).ok(),
            index: None,
            nb_matches: None,
        })
    }

    pub fn complete(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn leave(&mut self, status: &mut Status) -> Result<()> {
        match status.current_tab().display_mode {
            Display::Tree => {
                self.tree(&mut status.current_tab_mut().tree);
            }
            Display::Directory => {
                self.directory(status.current_tab_mut())?;
            }
            Display::Flagged => self.flagged(&mut status.menu.flagged),
            _ => (),
        };
        status.update_second_pane_for_preview()
    }

    pub fn directory(&mut self, tab: &mut Tab) -> Result<()> {
        if let Some(re) = &self.regex {
            let next_index = tab.directory.index;
            tab.search_from(re, next_index);
        }
        Ok(())
    }

    pub fn tree(&mut self, tree: &mut Tree) {
        let path = self.tree_find_next_path(tree).to_owned();
        tree.go(To::Path(&path));
    }

    fn tree_find_next_path<'a>(&mut self, tree: &'a mut Tree) -> &'a std::path::Path {
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

    pub fn flagged(&mut self, flagged: &mut Flagged) {
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
}
