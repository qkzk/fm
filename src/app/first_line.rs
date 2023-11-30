use anyhow::{Context, Result};
use unicode_segmentation::UnicodeSegmentation;

use crate::app::{Status, Tab};
use crate::event::ActionMap;
use crate::modes::{shorten_path, Display, SelectableContent};

/// Action for every element of the first line.
/// It should match the order of the `FirstLine::make_string` static method.
const ACTIONS: [ActionMap; 9] = [
    ActionMap::Goto,
    ActionMap::Rename,
    ActionMap::Nothing, // position
    ActionMap::Ncdu,
    ActionMap::Sort,
    ActionMap::LazyGit,
    ActionMap::Jump,
    ActionMap::Sort,
    ActionMap::Nothing, // for out of bounds
];

/// A bunch of strings displaying the status of the current directory.
/// It provides an `action` method to make the first line clickable.
pub struct FirstLine {
    strings: Vec<String>,
    sizes: Vec<usize>,
    width: usize,
}

// should it be held somewhere ?
impl FirstLine {
    /// Create the strings associated with the selected tab directory
    pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
        let (width, _) = status.internal_settings.term.term_size()?;
        let disk_space = status.disk_spaces_of_selected();
        let strings = Self::make_strings(status, tab, disk_space)?;
        let sizes = Self::make_sizes(&strings);

        Ok(Self {
            strings,
            sizes,
            width,
        })
    }

    // TODO! refactor using a `struct thing { string, start, end, action }`
    /// Returns a bunch of displayable strings.
    /// Watchout:
    /// 1. the length of the vector MUST BE the length of `ACTIONS` minus one.
    /// 2. the order must be respected.
    fn make_strings(status: &Status, tab: &Tab, disk_space: String) -> Result<Vec<String>> {
        Ok(vec![
            Self::string_shorten_path(tab)?,
            Self::string_first_row_selected_file(tab)?,
            Self::string_first_row_position(tab)?,
            Self::string_used_space(tab),
            Self::string_disk_space(&disk_space),
            Self::string_git_string(tab)?,
            Self::string_first_row_flags(status),
            Self::string_sort_kind(tab),
        ])
    }

    /// Returns the lengths of every displayed string.
    /// It uses `unicode_segmentation::UnicodeSegmentation::graphemes`
    /// to measure used space.
    /// It's not the number of bytes used since those strings may contain
    /// any UTF-8 grapheme.
    fn make_sizes(strings: &[String]) -> Vec<usize> {
        strings
            .iter()
            .map(|s| s.graphemes(true).collect::<Vec<&str>>().iter().len())
            .collect()
    }

    /// Vector of displayed strings.
    pub fn strings(&self) -> &Vec<String> {
        self.strings.as_ref()
    }

    /// Action for each associated file.
    pub fn action(&self, col: usize, is_right: bool) -> &ActionMap {
        let mut sum = 0;
        let offset = self.offset(is_right);
        for (index, size) in self.sizes.iter().enumerate() {
            sum += size;
            if col <= sum + offset {
                return &ACTIONS[index];
            }
        }
        &ActionMap::Nothing
    }

    fn offset(&self, is_right: bool) -> usize {
        if is_right {
            self.width / 2 + 2
        } else {
            1
        }
    }

    fn string_shorten_path(tab: &Tab) -> Result<String> {
        Ok(format!(" {}", shorten_path(&tab.path_content.path, None)?))
    }

    fn string_first_row_selected_file(tab: &Tab) -> Result<String> {
        match tab.display_mode {
            Display::Tree => Ok(format!(
                "/{rel}",
                rel = shorten_path(tab.tree.selected_path_relative_to_root()?, Some(18))?
            )),
            _ => {
                if let Some(fileinfo) = tab.path_content.selected() {
                    Ok(fileinfo.filename_without_dot_dotdot())
                } else {
                    Ok("".to_owned())
                }
            }
        }
    }

    fn string_first_row_position(tab: &Tab) -> Result<String> {
        let len: usize;
        let index: usize;
        if matches!(tab.display_mode, Display::Tree) {
            index = tab.tree.selected_node().context("no node")?.index() + 1;
            len = tab.tree.len();
        } else {
            index = tab.path_content.index + 1;
            len = tab.path_content.len();
        }
        Ok(format!(" {index} / {len} "))
    }

    fn string_used_space(tab: &Tab) -> String {
        format!(" {} ", tab.path_content.used_space())
    }

    fn string_disk_space(disk_space: &str) -> String {
        format!(" Avail: {disk_space} ")
    }

    fn string_git_string(tab: &Tab) -> Result<String> {
        Ok(format!(" {} ", tab.path_content.git_string()?))
    }

    fn string_sort_kind(tab: &Tab) -> String {
        format!(" {} ", &tab.settings.sort_kind)
    }

    fn string_first_row_flags(status: &Status) -> String {
        let nb_flagged = status.menu.flagged.len();
        let flag_string = if nb_flagged > 1 { "flags" } else { "flag" };
        format!(" {nb_flagged} {flag_string} ",)
    }
}
