mod inner {
    use anyhow::{Context, Result};

    use crate::app::{Status, Tab};
    use crate::common::{
        PathShortener, UtfWidth, HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE, LAZYGIT,
        LOG_FIRST_SENTENCE, LOG_SECOND_SENTENCE, NCDU,
    };
    use crate::event::ActionMap;
    use crate::modes::{Content, Display, FilterKind, Preview, Search, Selectable, Text, TextKind};

    #[derive(Clone, Copy)]
    pub enum Align {
        Left,
        Right,
    }

    /// A footer or header element that can be clicked
    ///
    /// Holds a text and an action.
    /// It knows where it's situated on the line
    #[derive(Clone)]
    pub struct ClickableString {
        text: String,
        action: ActionMap,
        width: usize,
        left: usize,
        right: usize,
    }

    impl ClickableString {
        /// Creates a new `ClickableString`.
        /// It calculates its position with `col` and `align`.
        /// If left aligned, the text size will be added to `col` and the text will span from col to col + width.
        /// otherwise, the text will spawn from col - width to col.
        fn new(text: String, align: Align, action: ActionMap, col: usize) -> Self {
            let width = text.utf_width();
            let (left, right) = match align {
                Align::Left => (col, col + width),
                Align::Right => (col - width - 3, col - 3),
            };
            Self {
                text,
                action,
                width,
                left,
                right,
            }
        }

        /// Text content of the element.
        pub fn text(&self) -> &str {
            self.text.as_str()
        }

        pub fn col(&self) -> usize {
            self.left
        }

        pub fn width(&self) -> usize {
            self.width
        }
    }

    /// A line of element that can be clicked on.
    pub trait ClickableLine {
        /// Reference to the elements
        fn elems(&self) -> &Vec<ClickableString>;
        /// Action for each associated file.
        fn action(&self, col: usize, is_right: bool) -> &ActionMap {
            let offset = self.offset(is_right);
            let col = col - offset;
            for clickable in self.elems().iter() {
                if clickable.left <= col && col < clickable.right {
                    return &clickable.action;
                }
            }

            crate::log_info!("no action found");
            &ActionMap::Nothing
        }
        /// Full width of the terminal
        fn full_width(&self) -> usize;
        /// canvas width of the window
        fn canvas_width(&self) -> usize;
        /// used offset.
        /// 1 if the text is on left tab,
        /// width / 2 + 2 otherwise.
        fn offset(&self, is_right: bool) -> usize {
            if is_right {
                self.full_width() / 2 + 2
            } else {
                1
            }
        }
    }

    /// Header for tree & directory display mode.
    pub struct Header {
        elems: Vec<ClickableString>,
        canvas_width: usize,
        full_width: usize,
    }

    impl Header {
        /// Creates a new header
        pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
            let full_width = status.internal_settings.term_size().0;
            let canvas_width = status.canvas_width()?;
            let elems = Self::make_elems(tab, canvas_width)?;

            Ok(Self {
                elems,
                canvas_width,
                full_width,
            })
        }

        fn make_elems(tab: &Tab, width: usize) -> Result<Vec<ClickableString>> {
            let mut left = 0;
            let mut right = width;
            let shorten_path = Self::elem_shorten_path(tab, left)?;
            left += shorten_path.width();

            let filename = Self::elem_filename(tab, width, left)?;

            let mut elems = vec![shorten_path, filename];

            if !tab.search.is_empty() {
                let search = Self::elem_search(&tab.search, right);
                right -= search.width();
                elems.push(search);
            }

            let filter_kind = &tab.settings.filter;
            if !matches!(filter_kind, FilterKind::All) {
                let filter = Self::elem_filter(filter_kind, right);
                elems.push(filter);
            }

            Ok(elems)
        }

        fn elem_shorten_path(tab: &Tab, left: usize) -> Result<ClickableString> {
            Ok(ClickableString::new(
                format!(
                    " {}",
                    PathShortener::path(&tab.directory.path)
                        .context("Couldn't parse path")?
                        .shorten()
                ),
                Align::Left,
                ActionMap::Cd,
                left,
            ))
        }

        fn elem_filename(tab: &Tab, width: usize, left: usize) -> Result<ClickableString> {
            let text = match tab.display_mode {
                Display::Tree => Self::elem_tree_filename(tab, width)?,
                _ => Self::elem_directory_filename(tab),
            };
            Ok(ClickableString::new(
                text,
                Align::Left,
                ActionMap::Rename,
                left,
            ))
        }

        fn elem_tree_filename(tab: &Tab, width: usize) -> Result<String> {
            Ok(format!(
                "{sep}{rel}",
                rel = PathShortener::path(tab.tree.selected_path_relative_to_root()?)
                    .context("Couldn't parse path")?
                    .with_size(width / 2)
                    .shorten(),
                sep = if tab.tree.root_path() == std::path::Path::new("/") {
                    ""
                } else {
                    "/"
                }
            ))
        }

        fn elem_directory_filename(tab: &Tab) -> String {
            if tab.directory.is_dotdot_selected() {
                "".to_owned()
            } else if let Some(fileinfo) = tab.directory.selected() {
                fileinfo.filename_without_dot_dotdot()
            } else {
                "".to_owned()
            }
        }

        fn elem_search(search: &Search, right: usize) -> ClickableString {
            ClickableString::new(search.to_string(), Align::Right, ActionMap::Search, right)
        }

        fn elem_filter(filter: &FilterKind, right: usize) -> ClickableString {
            ClickableString::new(format!(" {filter}"), Align::Right, ActionMap::Filter, right)
        }
    }

    impl ClickableLine for Header {
        fn elems(&self) -> &Vec<ClickableString> {
            &self.elems
        }
        fn canvas_width(&self) -> usize {
            self.canvas_width
        }
        fn full_width(&self) -> usize {
            self.full_width
        }
    }

    /// Default footer for display directory & tree.
    pub struct Footer {
        elems: Vec<ClickableString>,
        canvas_width: usize,
        full_width: usize,
    }

    impl ClickableLine for Footer {
        fn elems(&self) -> &Vec<ClickableString> {
            &self.elems
        }
        fn canvas_width(&self) -> usize {
            self.canvas_width
        }

        fn full_width(&self) -> usize {
            self.full_width
        }
    }

    impl Footer {
        fn footer_actions() -> [ActionMap; 6] {
            [
                ActionMap::Nothing, // position
                ActionMap::Custom("%t ".to_owned() + NCDU),
                ActionMap::Sort,
                ActionMap::Custom("%t ".to_owned() + LAZYGIT),
                ActionMap::DisplayFlagged,
                ActionMap::Sort,
            ]
        }

        /// Creates a new footer
        pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
            let full_width = status.internal_settings.term_size().0;
            let canvas_width = status.canvas_width().unwrap();
            let elems = Self::make_elems(status, tab, canvas_width)?;
            Ok(Self {
                elems,
                canvas_width,
                full_width,
            })
        }

        fn make_elems(status: &Status, tab: &Tab, width: usize) -> Result<Vec<ClickableString>> {
            let disk_space = status.disk_spaces_of_selected();
            let raw_strings = Self::make_raw_strings(status, tab, disk_space)?;
            let padded_strings = Self::make_padded_strings(&raw_strings, width);
            let mut left = 0;
            let mut elems = vec![];
            for (index, string) in padded_strings.iter().enumerate() {
                let elem = ClickableString::new(
                    string.to_owned(),
                    Align::Left,
                    Self::footer_actions()[index].to_owned(),
                    left,
                );
                left += elem.width();
                elems.push(elem)
            }
            Ok(elems)
        }

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
            let used_width: usize = raw_strings.iter().map(|s| s.utf_width()).sum();
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
            if tab.display_mode.is_tree() {
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

    /// Header for the display of flagged files
    pub struct FlaggedHeader {
        elems: Vec<ClickableString>,
        canvas_width: usize,
        full_width: usize,
    }

    impl ClickableLine for FlaggedHeader {
        fn elems(&self) -> &Vec<ClickableString> {
            &self.elems
        }
        fn canvas_width(&self) -> usize {
            self.canvas_width
        }
        fn full_width(&self) -> usize {
            self.full_width
        }
    }

    impl FlaggedHeader {
        const ACTIONS: [ActionMap; 3] =
            [ActionMap::ResetMode, ActionMap::OpenFile, ActionMap::Search];

        /// Creates a new header.
        pub fn new(status: &Status) -> Result<Self> {
            let full_width = status.internal_settings.term_size().0;
            let canvas_width = status.canvas_width()?;
            let elems = Self::make_elems(status, full_width);

            Ok(Self {
                elems,
                canvas_width,
                full_width,
            })
        }

        fn make_elems(status: &Status, width: usize) -> Vec<ClickableString> {
            let title = ClickableString::new(
                "Fuzzy files".to_owned(),
                Align::Left,
                Self::ACTIONS[0].to_owned(),
                0,
            );
            let left = title.width();

            let flagged = ClickableString::new(
                status
                    .menu
                    .flagged
                    .selected()
                    .unwrap_or(&std::path::PathBuf::new())
                    .to_string_lossy()
                    .to_string(),
                Align::Left,
                Self::ACTIONS[1].to_owned(),
                left,
            );
            let searched = Header::elem_search(&status.current_tab().search, width);
            vec![title, flagged, searched]
        }
    }

    /// Footer for the flagged files display
    pub struct FlaggedFooter {
        elems: Vec<ClickableString>,
        canvas_width: usize,
        full_width: usize,
    }

    impl ClickableLine for FlaggedFooter {
        fn elems(&self) -> &Vec<ClickableString> {
            &self.elems
        }
        fn canvas_width(&self) -> usize {
            self.canvas_width
        }
        fn full_width(&self) -> usize {
            self.full_width
        }
    }

    impl FlaggedFooter {
        const ACTIONS: [ActionMap; 2] = [ActionMap::Nothing, ActionMap::DisplayFlagged];

        /// Creates a new footer
        pub fn new(status: &Status) -> Result<Self> {
            let full_width = status.internal_settings.term_size().0;
            let canvas_width = status.canvas_width()?;
            let raw_strings = Self::make_strings(status);
            let strings = Footer::make_padded_strings(&raw_strings, full_width);
            let elems = Self::make_elems(strings);

            Ok(Self {
                elems,
                canvas_width,
                full_width,
            })
        }

        fn make_elems(padded_strings: Vec<String>) -> Vec<ClickableString> {
            let mut elems = vec![];
            let mut left = 0;
            for (index, string) in padded_strings.iter().enumerate() {
                let elem = ClickableString::new(
                    string.to_owned(),
                    Align::Left,
                    Self::ACTIONS[index].to_owned(),
                    left,
                );
                left += elem.width();
                elems.push(elem)
            }
            elems
        }

        fn make_strings(status: &Status) -> Vec<String> {
            let index = if status.menu.flagged.is_empty() {
                0
            } else {
                status.menu.flagged.index + 1
            };
            vec![
                format!(" {index} / {len}", len = status.menu.flagged.len()),
                format!(" {nb} flags", nb = status.menu.flagged.len()),
            ]
        }
    }

    pub struct PreviewHeader;

    impl PreviewHeader {
        pub fn elems(status: &Status, tab: &Tab, width: usize) -> Vec<ClickableString> {
            let pairs = Self::strings(status, tab);
            Self::pair_to_clickable(&pairs, width)
        }

        fn pair_to_clickable(pairs: &[(String, Align)], width: usize) -> Vec<ClickableString> {
            let mut left = 0;
            let mut right = width;
            let mut elems = vec![];
            for (text, align) in pairs.iter() {
                let pos = if let Align::Left = align { left } else { right };
                let elem = ClickableString::new(
                    text.to_owned(),
                    align.to_owned(),
                    ActionMap::Nothing,
                    pos,
                );
                match align {
                    Align::Left => {
                        left += elem.width();
                    }
                    Align::Right => {
                        right -= elem.width();
                    }
                }
                elems.push(elem)
            }
            elems
        }

        fn strings(status: &Status, tab: &Tab) -> Vec<(String, Align)> {
            match &tab.preview {
                Preview::Text(text_content) => match text_content.kind {
                    TextKind::CommandStdout => Self::make_colored_text(text_content),
                    TextKind::Help => Self::make_help(),
                    TextKind::Log => Self::make_log(),
                    _ => Self::make_default_preview(status, tab),
                },
                _ => Self::make_default_preview(status, tab),
            }
        }

        fn make_help() -> Vec<(String, Align)> {
            vec![
                (HELP_FIRST_SENTENCE.to_owned(), Align::Left),
                (
                    format!(" Version: {v} ", v = std::env!("CARGO_PKG_VERSION")),
                    Align::Left,
                ),
                (HELP_SECOND_SENTENCE.to_owned(), Align::Right),
            ]
        }

        fn make_log() -> Vec<(String, Align)> {
            vec![
                (LOG_FIRST_SENTENCE.to_owned(), Align::Left),
                (LOG_SECOND_SENTENCE.to_owned(), Align::Right),
            ]
        }

        fn make_colored_text(colored_text: &Text) -> Vec<(String, Align)> {
            vec![
                (" Command: ".to_owned(), Align::Left),
                (
                    format!(" {command} ", command = colored_text.title),
                    Align::Right,
                ),
            ]
        }

        fn _pick_previewed_fileinfo(status: &Status) -> String {
            if status.display_settings.dual() && status.display_settings.preview() {
                status.tabs[1].preview.filepath()
            } else {
                status.current_tab().preview.filepath()
            }
        }

        fn make_default_preview(status: &Status, tab: &Tab) -> Vec<(String, Align)> {
            let filepath = Self::_pick_previewed_fileinfo(status);
            let mut strings = vec![
                (" Preview ".to_owned(), Align::Left),
                (format!(" {} ", filepath), Align::Left),
            ];
            if !tab.preview.is_empty() {
                let index = match &tab.preview {
                    Preview::Ueberzug(image) => image.index + 1,
                    _ => tab.window.bottom,
                };
                strings.push((
                    format!(" {index} / {len} ", len = tab.preview.len()),
                    Align::Right,
                ));
            };
            strings
        }

        /// Make a default preview header
        pub fn default_preview(status: &Status, tab: &Tab, width: usize) -> Vec<ClickableString> {
            Self::pair_to_clickable(&Self::make_default_preview(status, tab), width)
        }
    }
}

pub use inner::{
    ClickableLine, ClickableString, FlaggedFooter, FlaggedHeader, Footer, Header, PreviewHeader,
};
