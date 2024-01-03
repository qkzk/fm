mod inner {
    use anyhow::{Context, Result};
    use unicode_segmentation::UnicodeSegmentation;

    use crate::app::{Status, Tab};
    use crate::event::ActionMap;
    use crate::modes::Selectable;
    use crate::modes::{shorten_path, Display};
    use crate::modes::{Content, FilterKind};

    /// Action for every element of the first line.
    /// It should match the order of the `FirstLine::make_string` static method.
    const HEADER_ACTIONS: [ActionMap; 4] = [
        ActionMap::Cd,
        ActionMap::Rename,
        ActionMap::Search,
        ActionMap::Filter,
    ];

    const FOOTER_ACTIONS: [ActionMap; 7] = [
        ActionMap::Nothing, // position
        ActionMap::Ncdu,
        ActionMap::Sort,
        ActionMap::LazyGit,
        ActionMap::Jump,
        ActionMap::Sort,
        ActionMap::Nothing, // for out of bounds
    ];

    pub trait ClickableLine: ClickableLineInner {
        fn strings(&self) -> &Vec<String>;
        /// Action for each associated file.
        fn action(&self, col: usize, is_right: bool) -> &ActionMap {
            let mut sum = 0;
            let offset = self.offset(is_right);
            for (index, size) in self.sizes().iter().enumerate() {
                sum += size;
                if col <= sum + offset {
                    return self.action_index(index);
                }
            }
            &ActionMap::Nothing
        }
    }

    pub trait ClickableLineInner {
        fn width(&self) -> usize;
        fn sizes(&self) -> &Vec<usize>;
        fn action_index(&self, index: usize) -> &ActionMap;

        fn offset(&self, is_right: bool) -> usize {
            if is_right {
                self.width() / 2 + 2
            } else {
                1
            }
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
    }

    /// A bunch of strings displaying the status of the current directory.
    /// It provides an `action` method to make the first line clickable.
    pub struct Header {
        strings: Vec<String>,
        sizes: Vec<usize>,
        width: usize,
        actions: Vec<ActionMap>,
    }

    impl ClickableLine for Header {
        /// Vector of displayed strings.
        fn strings(&self) -> &Vec<String> {
            self.strings.as_ref()
        }
    }

    impl ClickableLineInner for Header {
        fn sizes(&self) -> &Vec<usize> {
            self.sizes.as_ref()
        }

        fn action_index(&self, index: usize) -> &ActionMap {
            &self.actions(index)
        }

        fn width(&self) -> usize {
            self.width
        }
    }

    // should it be held somewhere ?
    impl Header {
        /// Create the strings associated with the selected tab directory
        pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
            let (width, _) = status.internal_settings.term.term_size()?;
            let (strings, actions) = Self::make_strings_actions(tab, width)?;
            let sizes = Self::make_sizes(&strings);

            Ok(Self {
                strings,
                sizes,
                width,
                actions,
            })
        }

        fn actions(&self, index: usize) -> &ActionMap {
            &self.actions[index]
        }

        // TODO! refactor using a `struct thing { string, start, end, action }`
        /// Returns a bunch of displayable strings.
        /// Watchout:
        /// 1. the length of the vector MUST BE the length of `ACTIONS` minus one.
        /// 2. the order must be respected.
        fn make_strings_actions(tab: &Tab, width: usize) -> Result<(Vec<String>, Vec<ActionMap>)> {
            let mut strings = vec![
                Self::string_shorten_path(tab)?,
                Self::string_first_row_selected_file(tab, width)?,
            ];
            let mut actions: Vec<ActionMap> = HEADER_ACTIONS[0..2].into();
            if let Some(searched) = &tab.searched {
                strings.push(Self::string_searched(searched));
                actions.push(HEADER_ACTIONS[2].clone());
            }
            if !matches!(tab.settings.filter, FilterKind::All) {
                strings.push(Self::string_filter(tab));
                actions.push(HEADER_ACTIONS[3].clone());
            }
            Ok((strings, actions))
        }

        fn string_filter(tab: &Tab) -> String {
            format!(" {filter} ", filter = tab.settings.filter.to_string())
        }

        fn string_searched(searched: &str) -> String {
            format!(" Searched: {searched} ")
        }

        fn string_shorten_path(tab: &Tab) -> Result<String> {
            Ok(format!(" {}", shorten_path(&tab.directory.path, None)?))
        }

        fn string_first_row_selected_file(tab: &Tab, width: usize) -> Result<String> {
            match tab.display_mode {
                Display::Tree => Ok(format!(
                    "/{rel}",
                    rel =
                        shorten_path(tab.tree.selected_path_relative_to_root()?, Some(width / 2))?
                )),
                _ => {
                    if let Some(fileinfo) = tab.directory.selected() {
                        Ok(fileinfo.filename_without_dot_dotdot())
                    } else {
                        Ok("".to_owned())
                    }
                }
            }
        }
    }

    /// A clickable footer.
    /// Every displayed element knows were it starts and ends.
    /// It allows the user to click on them.
    /// Those element are linked by their index to an action.
    pub struct Footer {
        strings: Vec<String>,
        sizes: Vec<usize>,
        width: usize,
    }

    impl ClickableLine for Footer {
        /// Vector of displayed strings.
        fn strings(&self) -> &Vec<String> {
            self.strings.as_ref()
        }
    }

    impl ClickableLineInner for Footer {
        fn sizes(&self) -> &Vec<usize> {
            self.sizes.as_ref()
        }

        fn action_index(&self, index: usize) -> &ActionMap {
            &FOOTER_ACTIONS[index]
        }

        fn width(&self) -> usize {
            self.width
        }
    }

    impl Footer {
        /// Create the strings associated with the selected tab directory
        pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
            let (width, _) = status.internal_settings.term.term_size()?;
            let used_width = if status.display_settings.use_dual_tab(width) {
                width / 2
            } else {
                width
            };
            let disk_space = status.disk_spaces_of_selected();
            let raw_strings = Self::make_raw_strings(status, tab, disk_space)?;
            let strings = Self::make_padded_strings(&raw_strings, used_width);
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
        fn make_raw_strings(status: &Status, tab: &Tab, disk_space: String) -> Result<Vec<String>> {
            Ok(vec![
                Self::string_first_row_position(tab)?,
                Self::string_used_space(tab),
                Self::string_disk_space(&disk_space),
                Self::string_git_string(tab)?,
                Self::string_first_row_flags(status),
                Self::string_sort_kind(tab),
            ])
        }

        /// Pad every string of `raw_strings` with enough space to fill a line.
        fn make_padded_strings(raw_strings: &[String], total_width: usize) -> Vec<String> {
            let used_width: usize = raw_strings
                .iter()
                .map(|s| s.graphemes(true).collect::<Vec<&str>>().iter().len())
                .sum();
            let available_width = total_width.checked_sub(used_width).unwrap_or_default();
            let margin_width = available_width / (2 * raw_strings.len());
            let margin = " ".repeat(margin_width);
            raw_strings
                .iter()
                .map(|content| format!("{margin}{content}{margin}"))
                .collect()
        }

        fn string_first_row_position(tab: &Tab) -> Result<String> {
            let len: usize;
            let index: usize;
            if matches!(tab.display_mode, Display::Tree) {
                index = tab.tree.selected_node().context("no node")?.index() + 1;
                len = tab.tree.len();
            } else {
                index = tab.directory.index + 1;
                len = tab.directory.len();
            }
            Ok(format!(" {index} / {len} "))
        }

        fn string_used_space(tab: &Tab) -> String {
            format!(" {} ", tab.directory.used_space())
        }

        fn string_disk_space(disk_space: &str) -> String {
            format!(" Avail: {disk_space} ")
        }

        fn string_git_string(tab: &Tab) -> Result<String> {
            Ok(format!(" {} ", tab.directory.git_string()?))
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

    pub struct FuzzyHeader {
        strings: Vec<String>,
        actions: Vec<ActionMap>,
        sizes: Vec<usize>,
        width: usize,
    }

    impl FuzzyHeader {
        pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
            let strings = Self::make_strings(tab);
            let sizes = Self::make_sizes(&strings);
            let (width, _) = status.internal_settings.term.term_size()?;
            let actions = vec![ActionMap::ResetMode, ActionMap::OpenFile];

            Ok(Self {
                strings,
                sizes,
                width,
                actions,
            })
        }

        fn make_strings(tab: &Tab) -> Vec<String> {
            vec![
                "Fuzzy files".to_owned(),
                tab.fuzzy
                    .selected()
                    .unwrap_or(&std::path::PathBuf::new())
                    .to_string_lossy()
                    .to_string(),
            ]
        }

        fn make_sizes(strings: &[String]) -> Vec<usize> {
            strings.iter().map(|s| s.len()).collect()
        }

        fn actions(&self, index: usize) -> &ActionMap {
            &self.actions[index]
        }
    }
    impl ClickableLine for FuzzyHeader {
        /// Vector of displayed strings.
        fn strings(&self) -> &Vec<String> {
            self.strings.as_ref()
        }
    }

    impl ClickableLineInner for FuzzyHeader {
        fn sizes(&self) -> &Vec<usize> {
            self.sizes.as_ref()
        }

        fn action_index(&self, index: usize) -> &ActionMap {
            &self.actions(index)
        }

        fn width(&self) -> usize {
            self.width
        }
    }
}

pub use inner::{ClickableLine, Footer, FuzzyHeader, Header};
