mod inner {
    use anyhow::{Context, Result};

    use crate::app::{Status, Tab};
    use crate::common::{
        UtfWidth, HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE, LOG_FIRST_SENTENCE,
        LOG_SECOND_SENTENCE,
    };
    use crate::event::ActionMap;
    use crate::modes::{
        shorten_path, ColoredText, Content, Display, FileInfo, FilterKind, Preview, Search,
        Selectable, TextKind,
    };

    #[derive(Clone, Copy)]
    pub enum HorizontalAlign {
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
        fn new(text: String, align: HorizontalAlign, action: ActionMap, col: usize) -> Self {
            let width = text.utf_width();
            let (left, right) = match align {
                HorizontalAlign::Left => (col, col + width),
                HorizontalAlign::Right => (col - width - 3, col - 3),
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
            let full_width = status.internal_settings.term_size()?.0;
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
                format!(" {}", shorten_path(&tab.directory.path, None)?),
                HorizontalAlign::Left,
                ActionMap::Cd,
                left,
            ))
        }

        fn elem_filename(tab: &Tab, width: usize, left: usize) -> Result<ClickableString> {
            let text = match tab.display_mode {
                Display::Tree => format!(
                    "/{rel}",
                    rel =
                        shorten_path(tab.tree.selected_path_relative_to_root()?, Some(width / 2))?
                ),
                _ => {
                    if let Some(fileinfo) = tab.directory.selected() {
                        fileinfo.filename_without_dot_dotdot()
                    } else {
                        "".to_owned()
                    }
                }
            };
            Ok(ClickableString::new(
                text,
                HorizontalAlign::Left,
                ActionMap::Rename,
                left,
            ))
        }

        fn elem_search(search: &Search, right: usize) -> ClickableString {
            ClickableString::new(
                search.to_string(),
                HorizontalAlign::Right,
                ActionMap::Search,
                right,
            )
        }

        fn elem_filter(filter: &FilterKind, right: usize) -> ClickableString {
            ClickableString::new(
                format!(" {filter}"),
                HorizontalAlign::Right,
                ActionMap::Filter,
                right,
            )
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
        const FOOTER_ACTIONS: [ActionMap; 6] = [
            ActionMap::Nothing, // position
            ActionMap::Ncdu,
            ActionMap::Sort,
            ActionMap::LazyGit,
            ActionMap::DisplayFlagged,
            ActionMap::Sort,
        ];

        /// Creates a new footer
        pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
            let full_width = status.internal_settings.term_size()?.0;
            let canvas_width = status.canvas_width()?;
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
                    HorizontalAlign::Left,
                    Self::FOOTER_ACTIONS[index].to_owned(),
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
            let full_width = status.internal_settings.term_size()?.0;
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
                HorizontalAlign::Left,
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
                HorizontalAlign::Left,
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
            let full_width = status.internal_settings.term.term_size()?.0;
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
                    HorizontalAlign::Left,
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

        fn pair_to_clickable(
            pairs: &[(String, HorizontalAlign)],
            width: usize,
        ) -> Vec<ClickableString> {
            let mut left = 0;
            let mut right = width;
            let mut elems = vec![];
            for (text, align) in pairs.iter() {
                let pos = if let HorizontalAlign::Left = align {
                    left
                } else {
                    right
                };
                let elem = ClickableString::new(
                    text.to_owned(),
                    align.to_owned(),
                    ActionMap::Nothing,
                    pos,
                );
                match align {
                    HorizontalAlign::Left => {
                        left += elem.width();
                    }
                    HorizontalAlign::Right => {
                        right -= elem.width();
                    }
                }
                elems.push(elem)
            }
            elems
        }

        fn strings(status: &Status, tab: &Tab) -> Vec<(String, HorizontalAlign)> {
            match &tab.preview {
                Preview::Text(text_content) => match text_content.kind {
                    TextKind::HELP => Self::make_help(),
                    TextKind::LOG => Self::make_log(),
                    _ => Self::make_default_preview(status, tab),
                },
                Preview::ColoredText(colored_text) => Self::make_colored_text(colored_text),
                _ => Self::make_default_preview(status, tab),
            }
        }

        fn make_help() -> Vec<(String, HorizontalAlign)> {
            vec![
                (HELP_FIRST_SENTENCE.to_owned(), HorizontalAlign::Left),
                (
                    format!(" Version: {v} ", v = std::env!("CARGO_PKG_VERSION")),
                    HorizontalAlign::Left,
                ),
                (HELP_SECOND_SENTENCE.to_owned(), HorizontalAlign::Right),
            ]
        }

        fn make_log() -> Vec<(String, HorizontalAlign)> {
            vec![
                (LOG_FIRST_SENTENCE.to_owned(), HorizontalAlign::Left),
                (LOG_SECOND_SENTENCE.to_owned(), HorizontalAlign::Right),
            ]
        }

        fn make_colored_text(colored_text: &ColoredText) -> Vec<(String, HorizontalAlign)> {
            vec![
                (" Command: ".to_owned(), HorizontalAlign::Left),
                (
                    format!(" {command} ", command = colored_text.title()),
                    HorizontalAlign::Right,
                ),
            ]
        }

        fn _pick_previewed_fileinfo(status: &Status) -> Result<FileInfo> {
            if status.display_settings.dual() && status.display_settings.preview() {
                status.tabs[0].current_file()
            } else {
                status.current_tab().current_file()
            }
        }

        fn make_default_preview(status: &Status, tab: &Tab) -> Vec<(String, HorizontalAlign)> {
            if let Ok(fileinfo) = Self::_pick_previewed_fileinfo(status) {
                let mut strings = vec![(" Preview ".to_owned(), HorizontalAlign::Left)];
                if !tab.preview.is_empty() {
                    let index = match &tab.preview {
                        Preview::Ueberzug(image) => image.index + 1,
                        _ => tab.window.bottom,
                    };
                    strings.push((
                        format!(" {index} / {len} ", len = tab.preview.len()),
                        HorizontalAlign::Right,
                    ));
                };
                strings.push((
                    format!(" {} ", fileinfo.path.display()),
                    HorizontalAlign::Left,
                ));
                strings
            } else {
                vec![("".to_owned(), HorizontalAlign::Left)]
            }
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
