mod inner {
    use anyhow::{Context, Result};
    use ratatui::{
        layout::{Alignment, Rect},
        style::Modifier,
        text::{Line, Span},
        widgets::Widget,
        Frame,
    };

    use crate::common::{
        PathShortener, UtfWidth, ACTION_LOG_PATH, HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE,
        LAZYGIT, LOG_FIRST_SENTENCE, LOG_SECOND_SENTENCE, NCDU,
    };
    use crate::event::ActionMap;
    use crate::modes::{Content, Display, FilterKind, Preview, Search, Selectable, Text, TextKind};
    use crate::{
        app::{Status, Tab},
        config::MENU_STYLES,
    };

    /// Should the content be aligned left or right ? No centering.
    #[derive(Clone, Copy, Debug)]
    enum Align {
        Left,
        Right,
    }

    /// A footer or header element that can be clicked
    ///
    /// Holds a text and an action.
    /// It knows where it's situated on the line
    #[derive(Clone, Debug)]
    pub struct ClickableString {
        text: String,
        action: ActionMap,
        width: u16,
        left: u16,
        right: u16,
    }

    impl ClickableString {
        /// Creates a new `ClickableString`.
        /// It calculates its position with `col` and `align`.
        /// If left aligned, the text size will be added to `col` and the text will span from col to col + width.
        /// otherwise, the text will spawn from col - width to col.
        fn new(text: String, align: Align, action: ActionMap, col: u16) -> Self {
            let width = text.utf_width_u16();
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

        pub fn width(&self) -> u16 {
            self.width
        }
    }

    trait ToLine<'a> {
        fn left_to_line(&'a self, effect_reverse: bool) -> Line<'a>;
        fn right_to_line(&'a self, effect_reverse: bool) -> Line<'a>;
    }

    impl<'a> ToLine<'a> for &Vec<ClickableString> {
        fn left_to_line(&'a self, effect_reverse: bool) -> Line<'a> {
            let left: Vec<_> = std::iter::zip(
                self.iter(),
                MENU_STYLES
                    .get()
                    .expect("Menu colors should be set")
                    .palette()
                    .iter()
                    .cycle(),
            )
            .map(|(elem, style)| {
                let mut style = *style;
                if effect_reverse {
                    style.add_modifier |= Modifier::REVERSED;
                }
                Span::styled(elem.text(), style)
            })
            .collect();
            Line::from(left).alignment(Alignment::Left)
        }

        fn right_to_line(&'a self, effect_reverse: bool) -> Line<'a> {
            let left: Vec<_> = std::iter::zip(
                self.iter(),
                MENU_STYLES
                    .get()
                    .expect("Menu colors should be set")
                    .palette()
                    .iter()
                    .rev()
                    .cycle(),
            )
            .map(|(elem, style)| {
                let mut style = *style;
                if effect_reverse {
                    style.add_modifier |= Modifier::REVERSED;
                }
                Span::styled(elem.text(), style)
            })
            .collect();
            Line::from(left).alignment(Alignment::Right)
        }
    }

    /// A line of element that can be clicked on.
    pub trait ClickableLine {
        /// Reference to the elements
        fn left(&self) -> &Vec<ClickableString>;
        fn right(&self) -> &Vec<ClickableString>;
        /// Action for each associated file.
        fn action(&self, col: u16, is_right: bool) -> &ActionMap {
            let offset = self.offset(is_right);
            let col = col - offset;
            for clickable in self.left().iter().chain(self.right().iter()) {
                if clickable.left <= col && col < clickable.right {
                    return &clickable.action;
                }
            }

            crate::log_info!("no action found");
            &ActionMap::Nothing
        }
        /// Full width of the terminal
        fn full_width(&self) -> u16;
        /// used offset.
        /// 1 if the text is on left tab,
        /// width / 2 + 2 otherwise.
        fn offset(&self, is_right: bool) -> u16 {
            if is_right {
                self.full_width() / 2 + 2
            } else {
                1
            }
        }

        /// Draw the left aligned elements of the line.
        fn draw_left(&self, f: &mut Frame, rect: Rect, effect_reverse: bool) {
            self.left()
                .left_to_line(effect_reverse)
                .render(rect, f.buffer_mut());
        }

        /// Draw the right aligned elements of the line.
        fn draw_right(&self, f: &mut Frame, rect: Rect, effect_reverse: bool) {
            self.right()
                .right_to_line(effect_reverse)
                .render(rect, f.buffer_mut());
        }
    }

    /// Header for tree & directory display mode.
    pub struct Header {
        left: Vec<ClickableString>,
        right: Vec<ClickableString>,
        full_width: u16,
    }

    impl Header {
        /// Creates a new header
        pub fn new(status: &Status, tab: &Tab) -> Result<Self> {
            let full_width = status.internal_settings.term_size().0;
            let canvas_width = status.canvas_width()?;
            let left = Self::make_left(tab, canvas_width)?;
            let right = Self::make_right(tab, canvas_width)?;

            Ok(Self {
                left,
                right,
                full_width,
            })
        }

        fn make_left(tab: &Tab, width: u16) -> Result<Vec<ClickableString>> {
            let mut left = 0;
            let shorten_path = Self::elem_shorten_path(tab, left)?;
            left += shorten_path.width();

            let filename = Self::elem_filename(tab, width, left)?;

            Ok(vec![shorten_path, filename])
        }

        fn make_right(tab: &Tab, width: u16) -> Result<Vec<ClickableString>> {
            let mut right = width;
            let mut right_elems = vec![];

            if !tab.search.is_empty() {
                let search = Self::elem_search(&tab.search, right);
                right -= search.width();
                right_elems.push(search)
            }

            let filter_kind = &tab.settings.filter;
            if !matches!(filter_kind, FilterKind::All) {
                right_elems.push(Self::elem_filter(filter_kind, right))
            }

            Ok(right_elems)
        }

        fn elem_shorten_path(tab: &Tab, left: u16) -> Result<ClickableString> {
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

        fn elem_filename(tab: &Tab, width: u16, left: u16) -> Result<ClickableString> {
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

        fn elem_tree_filename(tab: &Tab, width: u16) -> Result<String> {
            Ok(format!(
                "{sep}{rel}",
                rel = PathShortener::path(tab.tree.selected_path_relative_to_root()?)
                    .context("Couldn't parse path")?
                    .with_size(width as usize / 2)
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

        fn elem_search(search: &Search, right: u16) -> ClickableString {
            ClickableString::new(search.to_string(), Align::Right, ActionMap::Search, right)
        }

        fn elem_filter(filter: &FilterKind, right: u16) -> ClickableString {
            ClickableString::new(format!(" {filter}"), Align::Right, ActionMap::Filter, right)
        }
    }

    static EMPTY_VEC: Vec<ClickableString> = vec![];

    impl ClickableLine for Header {
        fn left(&self) -> &Vec<ClickableString> {
            &self.left
        }
        fn right(&self) -> &Vec<ClickableString> {
            &self.right
        }
        fn full_width(&self) -> u16 {
            self.full_width
        }
    }

    /// Default footer for display directory & tree.
    pub struct Footer {
        left: Vec<ClickableString>,
        full_width: u16,
    }

    impl ClickableLine for Footer {
        fn left(&self) -> &Vec<ClickableString> {
            &self.left
        }
        fn right(&self) -> &Vec<ClickableString> {
            &EMPTY_VEC
        }

        fn full_width(&self) -> u16 {
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
            let canvas_width = status.canvas_width()?;
            let left = Self::make_elems(status, tab, canvas_width)?;
            Ok(Self { left, full_width })
        }

        fn make_elems(status: &Status, tab: &Tab, width: u16) -> Result<Vec<ClickableString>> {
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
        fn make_padded_strings(raw_strings: &[String], total_width: u16) -> Vec<String> {
            let total_width = total_width as usize;
            let used_width = raw_strings.iter().map(|s| s.utf_width()).sum();
            let available_width = total_width.saturating_sub(used_width);
            let margin_width = available_width / (2 * raw_strings.len());
            let margin = " ".repeat(margin_width);
            let mut padded_strings: Vec<String> = raw_strings
                .iter()
                .map(|content| format!("{margin}{content}{margin}"))
                .collect();
            let rest = total_width - padded_strings.iter().map(|s| s.utf_width()).sum::<usize>();
            padded_strings[raw_strings.len() - 1].push_str(&" ".repeat(rest));
            padded_strings
        }

        fn string_first_row_position(tab: &Tab) -> Result<String> {
            let len: u16;
            let index: u16;
            if tab.display_mode.is_tree() {
                index = tab.tree.selected_node().context("no node")?.index() as u16 + 1;
                len = tab.tree.len() as u16;
            } else {
                index = tab.directory.index as u16 + 1;
                len = tab.directory.len() as u16;
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

    /// Empty struct used to attach actions the first line of previews.
    /// What content ? What kind of preview ?
    pub struct PreviewHeader {
        left: Vec<ClickableString>,
        right: Vec<ClickableString>,
        full_width: u16,
    }

    impl ClickableLine for PreviewHeader {
        fn left(&self) -> &Vec<ClickableString> {
            &self.left
        }
        fn right(&self) -> &Vec<ClickableString> {
            &self.right
        }
        fn full_width(&self) -> u16 {
            self.full_width
        }
    }

    impl PreviewHeader {
        pub fn into_default_preview(status: &Status, tab: &Tab, width: u16) -> Self {
            Self {
                left: Self::default_preview(status, tab, width),
                right: vec![],
                full_width: width,
            }
        }

        pub fn new(status: &Status, tab: &Tab, width: u16) -> Self {
            Self {
                left: Self::pair_to_clickable(&Self::strings_left(status, tab), width),
                right: Self::pair_to_clickable(&Self::strings_right(tab), width),
                full_width: width,
            }
        }

        fn pair_to_clickable(pairs: &[(String, Align)], width: u16) -> Vec<ClickableString> {
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

        fn strings_left(status: &Status, tab: &Tab) -> Vec<(String, Align)> {
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

        fn strings_right(tab: &Tab) -> Vec<(String, Align)> {
            let index = match &tab.preview {
                Preview::Empty => 0,
                Preview::Ueberzug(image) => image.index + 1,
                _ => tab.window.bottom,
            };
            vec![(
                format!(" {index} / {len} ", len = tab.preview.len()),
                Align::Right,
            )]
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
                (ACTION_LOG_PATH.to_owned(), Align::Left),
                (LOG_SECOND_SENTENCE.to_owned(), Align::Right),
            ]
        }

        fn make_colored_text(colored_text: &Text) -> Vec<(String, Align)> {
            vec![
                (" Command output: ".to_owned(), Align::Left),
                (
                    format!(" {command} ", command = colored_text.title),
                    Align::Right,
                ),
            ]
        }

        fn pick_previewed_fileinfo(status: &Status) -> String {
            if status.session.dual() && status.session.preview() {
                status.tabs[1].preview.filepath()
            } else {
                status.current_tab().preview.filepath()
            }
        }

        fn make_default_preview(status: &Status, tab: &Tab) -> Vec<(String, Align)> {
            vec![
                (
                    format!(" Preview as {kind} ", kind = tab.preview.kind_display()),
                    Align::Left,
                ),
                (
                    format!(
                        " {filepath} ",
                        filepath = Self::pick_previewed_fileinfo(status)
                    ),
                    Align::Left,
                ),
            ]
        }

        /// Make a default preview header
        pub fn default_preview(status: &Status, tab: &Tab, width: u16) -> Vec<ClickableString> {
            Self::pair_to_clickable(&Self::make_default_preview(status, tab), width)
        }
    }
}

pub use inner::{ClickableLine, ClickableString, Footer, Header, PreviewHeader};
