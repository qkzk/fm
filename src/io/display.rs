use std::{io::Stdout, sync::MutexGuard};

use anyhow::{Context, Result};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Position, Rect, Size},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};

use crate::app::{ClickableLine, ClickableString, Footer, Header, PreviewHeader, Status, Tab};
use crate::common::path_to_string;
use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::{read_last_log_line, DrawMenu};
use crate::log_info;
use crate::modes::{
    parse_input_mode, BinaryContent, Content, ContentWindow, Display as DisplayMode, FileInfo,
    HLContent, InputSimple, LineDisplay, Menu as MenuMode, MoreInfos, Navigate, NeedConfirmation,
    Preview, SecondLine, Selectable, TLine, Text, TextKind, Trash, Tree, Ueber, Window,
};

pub trait Canvas: Sized {
    fn print_with_attr(&self, f: &mut Frame, row: u16, col: u16, content: &str, style: Style);

    fn print(&self, f: &mut Frame, row: u16, col: u16, content: &str);
}

impl Canvas for Rect {
    fn print_with_attr(&self, f: &mut Frame, row: u16, col: u16, content: &str, style: Style) {
        // Define the area for the text
        let area = Rect {
            x: self.x + col,
            y: self.y + row,
            width: content.len() as u16, // Set width based on content length
            height: 1,                   // One line of text
        };

        let paragraph = Paragraph::new(Line::from(vec![Span::styled(content, style)]));

        // Render at the specified coordinates
        f.render_widget(paragraph, area);
    }

    fn print(&self, f: &mut Frame, row: u16, col: u16, content: &str) {
        // Define the area for the text
        let area = Rect {
            x: self.x + col,
            y: self.y + row,
            width: content.len() as u16, // Set width based on content length
            height: 1,                   // One line of text
        };

        let paragraph = Paragraph::new(Line::from(vec![Span::styled(content, Style::default())]));

        // Render at the specified coordinates
        f.render_widget(paragraph, area);
    }
}

trait Draw {
    fn draw(&self, f: &mut Frame, rect: &Rect);
}

trait ClearLine {
    fn clear_line(&self, f: &mut Frame, row: u16);
}

impl ClearLine for Rect {
    fn clear_line(&self, f: &mut Frame, row: u16) {
        self.print(f, row, 0, &" ".repeat(self.width as usize));
    }
}

/// Iter over the content, returning a triplet of `(index, line, attr)`.
macro_rules! enumerated_colored_iter {
    ($t:ident) => {
        std::iter::zip(
            $t.iter().enumerate(),
            Gradient::new(
                ColorG::from_ratatui(
                    MENU_STYLES
                        .get()
                        .expect("Menu colors should be set")
                        .first
                        .fg
                        .unwrap_or(Color::Rgb(0, 0, 0)),
                )
                .unwrap_or_default(),
                ColorG::from_ratatui(
                    MENU_STYLES
                        .get()
                        .expect("Menu colors should be set")
                        .palette_3
                        .fg
                        .unwrap_or(Color::Rgb(0, 0, 0)),
                )
                .unwrap_or_default(),
                $t.len(),
            )
            .gradient()
            .map(|color| color_to_style(color)),
        )
        .map(|((index, line), attr)| (index, line, attr))
    };
}
/// Draw every line of the preview
macro_rules! impl_preview {
    ($f: ident, $text:ident, $tab:ident, $length:ident, $rect:ident, $line_number_width:ident, $window:ident, $height:ident) => {
        for (i, line) in (*$text).window($window.top, $window.bottom, $length) {
            let row = calc_line_row(i, $window);
            if row as usize > $height as usize {
                break;
            }
            $rect.print($f, row as u16, ($line_number_width + 3) as u16, line);
        }
    };
}

/// At least 120 chars width to display 2 tabs.
pub const MIN_WIDTH_FOR_DUAL_PANE: usize = 120;

enum TabPosition {
    Left,
    Right,
}

/// Bunch of attributes describing the state of a main window
/// relatively to other windows
struct FilesAttributes {
    /// horizontal position, in cells
    x_position: usize,
    /// is this the left or right window ?
    tab_position: TabPosition,
    /// is this tab selected ?
    is_selected: bool,
    /// is there a menuary window ?
    has_window_below: bool,
}

impl FilesAttributes {
    fn new(
        x_position: usize,
        tab_position: TabPosition,
        is_selected: bool,
        has_window_below: bool,
    ) -> Self {
        Self {
            x_position,
            tab_position,
            is_selected,
            has_window_below,
        }
    }

    fn is_right(&self) -> bool {
        matches!(self.tab_position, TabPosition::Right)
    }
}

struct FilesBuilder;

impl FilesBuilder {
    fn dual(status: &Status, width: usize) -> (Files, Files) {
        let first_selected = status.focus.is_left();
        let menu_selected = !first_selected;
        let attributes_left = FilesAttributes::new(
            0,
            TabPosition::Left,
            first_selected,
            status.tabs[0].need_menu_window(),
        );
        let files_left = Files::new(status, 0, attributes_left);
        let attributes_right = FilesAttributes::new(
            width / 2,
            TabPosition::Right,
            menu_selected,
            status.tabs[1].need_menu_window(),
        );
        let files_right = Files::new(status, 1, attributes_right);
        (files_left, files_right)
    }

    fn single(status: &Status) -> Files {
        let attributes_left = FilesAttributes::new(
            0,
            TabPosition::Left,
            true,
            status.tabs[0].need_menu_window(),
        );
        Files::new(status, 0, attributes_left)
    }
}

struct Files<'a> {
    status: &'a Status,
    tab: &'a Tab,
    attributes: FilesAttributes,
}

impl<'a> Draw for Files<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        if self.should_preview_in_right_tab() {
            self.preview_in_right_tab(f, rect);
            return;
        }
        FilesHeader::new(self.status, self.tab, self.attributes.is_selected)
            .unwrap()
            .draw(f, rect);
        self.copy_progress_bar(f, rect);
        self.content(f, rect);
        FilesFooter::new(self.status, self.tab, self.attributes.is_selected)
            .unwrap()
            .draw(f, rect);
    }
}

impl<'a> Files<'a> {
    fn new(status: &'a Status, index: usize, attributes: FilesAttributes) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
            attributes,
        }
    }

    fn should_preview_in_right_tab(&self) -> bool {
        self.status.display_settings.dual()
            && self.is_right()
            && self.status.display_settings.preview()
    }

    fn is_right(&self) -> bool {
        self.attributes.is_right()
    }

    fn content(&self, f: &mut Frame, rect: &Rect) {
        match &self.tab.display_mode {
            DisplayMode::Directory => DirectoryDisplay::new(self).draw(f, rect),
            DisplayMode::Tree => TreeDisplay::new(self).draw(f, rect),
            DisplayMode::Preview => PreviewDisplay::new(self).draw(f, rect),
        }
    }

    /// Display a copy progress bar on the left tab.
    /// Nothing is drawn if there's no copy atm.
    /// If the copy file queue has length > 1, we also display its size.
    fn copy_progress_bar(&self, f: &mut Frame, rect: &Rect) {
        if self.is_right() {
            return;
        }
        let Some(copy_progress) = &self.status.internal_settings.in_mem_progress else {
            return;
        };
        let progress_bar = copy_progress.contents();
        let nb_copy_left = self.status.internal_settings.copy_file_queue.len();
        let content = if nb_copy_left <= 1 {
            progress_bar
        } else {
            format!("{progress_bar}     -     1 of {nb}", nb = nb_copy_left)
        };
        rect.print_with_attr(
            f,
            1,
            2,
            &content,
            MENU_STYLES
                .get()
                .expect("Menu colors should be set")
                .palette_4,
        )
    }

    fn print_flagged_symbol(
        f: &mut Frame,
        status: &Status,
        rect: &Rect,
        row: usize,
        path: &std::path::Path,
        style: &mut Style,
    ) -> Result<()> {
        if status.menu.flagged.contains(path) {
            style.add_modifier |= Modifier::BOLD;
            rect.print_with_attr(
                f,
                row as u16,
                0,
                "█",
                MENU_STYLES.get().expect("Menu colors should be set").second,
            );
        }
        Ok(())
    }

    fn preview_in_right_tab(&self, f: &mut Frame, rect: &Rect) {
        let tab = &self.status.tabs[1];
        let _ = PreviewDisplay::new_with_args(self.status, tab, &self.attributes).draw(f, rect);
        let width = rect.width;
        draw_clickable_strings(
            f,
            0,
            0,
            &PreviewHeader::default_preview(self.status, tab, width as usize),
            rect,
            false,
        )
    }
}

struct DirectoryDisplay<'a> {
    status: &'a Status,
    tab: &'a Tab,
    attributes: &'a FilesAttributes,
}

impl<'a> Draw for DirectoryDisplay<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        self.files(f, rect)
    }
}

impl<'a> DirectoryDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
            tab: files.tab,
            attributes: &files.attributes,
        }
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Only the files around the selected one are displayed.
    /// We reverse the attributes of the selected one, underline the flagged files.
    /// When we display a simpler version, the menu line is used to display the
    /// metadata of the selected file.
    fn files(&self, f: &mut Frame, rect: &Rect) {
        let _ = FilesSecondLine::new(self.status, self.tab).draw(f, rect);
        self.files_content(f, rect);
        if !self.attributes.has_window_below && !self.attributes.is_right() {
            let _ = LogLine.draw(f, rect);
        }
    }

    fn files_content(&self, f: &mut Frame, rect: &Rect) {
        let (group_size, owner_size) = self.group_owner_size();
        let height = rect.height as u16;
        for (index, file) in self.tab.dir_enum_skip_take() {
            self.files_line(
                f,
                rect,
                group_size,
                owner_size,
                index,
                file,
                height as usize,
            );
        }
    }

    fn group_owner_size(&self) -> (usize, usize) {
        if self.status.display_settings.metadata() {
            (
                self.tab.directory.group_column_width(),
                self.tab.directory.owner_column_width(),
            )
        } else {
            (0, 0)
        }
    }

    fn files_line(
        &self,
        f: &mut Frame,
        rect: &Rect,
        group_size: usize,
        owner_size: usize,
        index: usize,
        file: &FileInfo,
        height: usize,
    ) {
        let row = index + ContentWindow::WINDOW_MARGIN_TOP - self.tab.window.top;
        if row > height {
            return;
        }
        let mut attr = file.style();
        if index == self.tab.directory.index {
            attr.add_modifier |= Modifier::REVERSED;
        }
        let content = Self::format_file_content(self.status, file, owner_size, group_size);
        Files::print_flagged_symbol(f, self.status, rect, row, &file.path, &mut attr);
        let col = 1 + self.status.menu.flagged.contains(&file.path) as usize;
        rect.print_with_attr(f, row as u16, col as u16, &content, attr);
    }

    fn format_file_content(
        status: &Status,
        file: &FileInfo,
        owner_size: usize,
        group_size: usize,
    ) -> String {
        if status.display_settings.metadata() {
            file.format(owner_size, group_size).unwrap()
        } else {
            file.format_simple().unwrap()
        }
    }
}

struct TreeDisplay<'a> {
    status: &'a Status,
    tab: &'a Tab,
}

impl<'a> Draw for TreeDisplay<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        self.tree(f, rect)
    }
}

impl<'a> TreeDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
            tab: files.tab,
        }
    }

    fn tree(&self, f: &mut Frame, rect: &Rect) {
        let _ = FilesSecondLine::new(self.status, self.tab).draw(f, rect);
        self.tree_content(f, rect)
    }

    fn tree_content(&self, f: &mut Frame, rect: &Rect) {
        let left_margin = 1;
        let height = rect.height;

        for (index, line_builder) in self.tab.tree.lines_enum_skip_take(&self.tab.window) {
            Self::tree_line(
                f,
                self.status,
                rect,
                line_builder,
                TreeLinePosition {
                    left_margin,
                    top: self.tab.window.top,
                    index,
                    height: height as usize,
                },
                self.status.display_settings.metadata(),
            );
        }
    }

    fn tree_line(
        f: &mut Frame,
        status: &Status,
        rect: &Rect,
        line_builder: &TLine,
        position_param: TreeLinePosition,
        with_medatadata: bool,
    ) -> Result<()> {
        let (mut col, top, index, height) = position_param.export();
        let row = (index + ContentWindow::WINDOW_MARGIN_TOP).saturating_sub(top);
        if row > height {
            return Ok(());
        }

        let mut style = line_builder.style;
        let path = line_builder.path();
        Files::print_flagged_symbol(f, status, rect, row, path, &mut style)?;

        col += Self::tree_metadata(f, rect, with_medatadata, row, col, line_builder, style);
        col += if index == 0 { 2 } else { 1 };
        col += line_builder.prefix().len();
        rect.print(f, row as u16, col as u16, line_builder.prefix());
        col += Self::tree_line_calc_flagged_offset(status, path);
        rect.print_with_attr(f, row as u16, col as u16, &line_builder.filename(), style);
        Ok(())
    }

    fn tree_metadata(
        f: &mut Frame,
        rect: &Rect,
        with_medatadata: bool,
        row: usize,
        col: usize,
        line_builder: &TLine,
        style: Style,
    ) -> usize {
        if with_medatadata {
            let len = line_builder.metadata().len();
            rect.print_with_attr(f, row as u16, col as u16, line_builder.metadata(), style);
            len
        } else {
            0
        }
    }

    fn tree_line_calc_flagged_offset(status: &Status, path: &std::path::Path) -> usize {
        status.menu.flagged.contains(path) as usize
    }
}

struct PreviewDisplay<'a> {
    status: &'a Status,
    tab: &'a Tab,
    attributes: &'a FilesAttributes,
}

/// Display a scrollable preview of a file.
/// Multiple modes are supported :
/// if the filename extension is recognized, the preview is highlighted,
/// if the file content is recognized as binary, an hex dump is previewed with 16 bytes lines,
/// else the content is supposed to be text and shown as such.
/// It may fail to recognize some usual extensions, notably `.toml`.
/// It may fail to recognize small files (< 1024 bytes).
impl<'a> Draw for PreviewDisplay<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let tab = self.tab;
        let window = &tab.window;
        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        let height = rect.height;
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                self.syntaxed(f, syntaxed, length, rect, line_number_width, window)
            }
            Preview::Binary(bin) => self.binary(f, bin, length, rect, window),
            Preview::Ueberzug(image) => self.ueberzug(image, rect),
            Preview::Tree(tree_preview) => self.tree_preview(f, tree_preview, window, rect),
            Preview::Text(colored_text) if matches!(colored_text.kind, TextKind::CommandStdout) => {
                self.colored_text(f, colored_text, length, rect, window)
            }
            Preview::Text(text) => {
                impl_preview!(
                    f,
                    text,
                    tab,
                    length,
                    rect,
                    line_number_width,
                    window,
                    height
                )
            }

            Preview::Empty => (),
        };
    }
}

impl<'a> PreviewDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
            tab: files.tab,
            attributes: &files.attributes,
        }
    }

    fn new_with_args(status: &'a Status, tab: &'a Tab, attributes: &'a FilesAttributes) -> Self {
        Self {
            status,
            tab,
            attributes,
        }
    }

    fn line_number(
        f: &mut Frame,
        row_position_in_rect: usize,
        line_number_to_print: usize,
        rect: &Rect,
    ) -> usize {
        let len = line_number_to_print.to_string().len();
        rect.print_with_attr(
            f,
            row_position_in_rect as u16,
            0,
            &line_number_to_print.to_string(),
            MENU_STYLES.get().expect("Menu colors should be set").first,
        );
        len
    }

    fn syntaxed(
        &self,
        f: &mut Frame,
        syntaxed: &HLContent,
        length: usize,
        rect: &Rect,
        line_number_width: usize,
        window: &ContentWindow,
    ) {
        for (i, vec_line) in (*syntaxed).window(window.top, window.bottom, length) {
            let row_position = calc_line_row(i, window);
            Self::line_number(f, row_position, i + 1, rect);
            for token in vec_line.iter() {
                todo!();
                // TODO! print method for syntaxed
                // token.print(f, rect, row_position, line_number_width);
            }
        }
    }

    fn binary(
        &self,
        f: &mut Frame,
        bin: &BinaryContent,
        length: usize,
        rect: &Rect,
        window: &ContentWindow,
    ) {
        let height = rect.height;
        let line_number_width_hex = format!("{:x}", bin.len() * 16).len();

        for (i, line) in (*bin).window(window.top, window.bottom, length) {
            let row = calc_line_row(i, window);
            if row as u16 > height {
                break;
            }
            rect.print_with_attr(
                f,
                row as u16,
                0,
                &format_line_nr_hex(i + 1 + window.top, line_number_width_hex),
                MENU_STYLES.get().expect("Menu colors should be set").first,
            );
            // TODO! binary print_ascii, print_bytes
            todo!("print bytes, print_ascii")
            // line.print_bytes(rect, row, line_number_width_hex + 1);
            // line.print_ascii(rect, row, line_number_width_hex + 43);
        }
    }

    fn ueberzug(&self, image: &Ueber, rect: &Rect) {
        let width = rect.width;
        let height = rect.height;
        // image.match_index()?;
        image.draw(
            self.attributes.x_position as u16 + 2,
            3,
            width as u16 - 2,
            height as u16 - 2,
        );
    }

    fn tree_preview(&self, f: &mut Frame, tree: &Tree, window: &ContentWindow, rect: &Rect) {
        let height = rect.height;
        let tree_content = tree.displayable();
        let content = tree_content.lines();
        let length = content.len();

        for (index, tree_line_builder) in
            tree.displayable().window(window.top, window.bottom, length)
        {
            TreeDisplay::tree_line(
                f,
                self.status,
                rect,
                tree_line_builder,
                TreeLinePosition {
                    left_margin: 0,
                    top: window.top,
                    index,
                    height: height as usize,
                },
                false,
            );
        }
    }

    fn colored_text(
        &self,
        f: &mut Frame,
        colored_text: &Text,
        length: usize,
        rect: &Rect,
        window: &ContentWindow,
    ) {
        let height = rect.height;
        for (i, line) in colored_text.window(window.top, window.bottom, length) {
            let row = calc_line_row(i, window);
            if row > height as usize {
                break;
            }
            let mut col = 3;
            // TODO!
            // skim ansi string parse...
            // for (chr, attr) in skim::AnsiString::parse(line).iter() {
            //     col += rect.print_with_attr(row, col, &chr.to_string(), attr)?;
            // }
        }
    }
}

struct TreeLinePosition {
    left_margin: usize,
    top: usize,
    index: usize,
    height: usize,
}

impl TreeLinePosition {
    /// left_margin, top, index, height
    fn export(&self) -> (usize, usize, usize, usize) {
        (self.left_margin, self.top, self.index, self.height)
    }
}

struct FilesHeader<'a> {
    status: &'a Status,
    tab: &'a Tab,
    is_selected: bool,
}

impl<'a> Draw for FilesHeader<'a> {
    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    /// Returns the result of the number of printed chars.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let width = rect.width;
        let content = match self.tab.display_mode {
            DisplayMode::Preview => PreviewHeader::elems(self.status, self.tab, width as usize),
            _ => Header::new(self.status, self.tab)
                .unwrap()
                .elems()
                .to_owned(),
        };
        draw_clickable_strings(f, 0, 0, &content, rect, self.is_selected);
    }
}

impl<'a> FilesHeader<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Result<Self> {
        Ok(Self {
            status,
            tab,
            is_selected,
        })
    }
}

#[derive(Default)]
struct FilesSecondLine {
    content: Option<String>,
    style: Option<Style>,
}

impl Draw for FilesSecondLine {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        match (&self.content, &self.style) {
            (Some(content), Some(style)) => {
                rect.print_with_attr(f, 1, 1, content, *style);
                content.len()
            }
            _ => 0,
        };
    }
}

impl FilesSecondLine {
    fn new(status: &Status, tab: &Tab) -> Self {
        if tab.display_mode.is_preview() || status.display_settings.metadata() {
            return Self::default();
        };
        if let Ok(file) = tab.current_file() {
            Self::second_line_detailed(&file)
        } else {
            Self::default()
        }
    }

    fn second_line_detailed(file: &FileInfo) -> Self {
        let owner_size = file.owner.len();
        let group_size = file.group.len();
        let mut attr = file.style();
        attr.add_modifier ^= Modifier::REVERSED;

        Self {
            content: Some(file.format(owner_size, group_size).unwrap_or_default()),
            style: Some(attr),
        }
    }
}

struct LogLine;

impl Draw for LogLine {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let height = rect.height;
        rect.print_with_attr(
            f,
            height - 2,
            4,
            &read_last_log_line(),
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
    }
}

struct FilesFooter<'a> {
    status: &'a Status,
    tab: &'a Tab,
    is_selected: bool,
}

impl<'a> Draw for FilesFooter<'a> {
    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    /// Returns the result of the number of printed chars.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let width = rect.width;
        let height = rect.height;
        let content = match self.tab.display_mode {
            DisplayMode::Preview => vec![],
            _ => Footer::new(self.status, self.tab)
                .unwrap()
                .elems()
                .to_owned(),
        };
        let mut style = MENU_STYLES.get().expect("Menu colors should be set").first;
        let last_index = (content.len().saturating_sub(1))
            % MENU_STYLES
                .get()
                .expect("Menu colors should be set")
                .palette_size();
        let mut background = MENU_STYLES
            .get()
            .expect("Menu colors should be set")
            .palette()[last_index];
        if self.is_selected {
            style.add_modifier |= Modifier::REVERSED;
            background.add_modifier |= Modifier::REVERSED;
        };
        rect.print_with_attr(f, height - 1, 0, &" ".repeat(width as usize), background);
        draw_clickable_strings(f, height as usize - 1, 0, &content, rect, self.is_selected);
    }
}

impl<'a> FilesFooter<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Result<Self> {
        Ok(Self {
            status,
            tab,
            is_selected,
        })
    }
}

struct Menu<'a> {
    status: &'a Status,
    tab: &'a Tab,
}

impl<'a> Draw for Menu<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let mode = self.tab.menu_mode;
        self.cursor(f);
        MenuFirstLine::new(self.status).draw(f, rect);
        self.menu_line(f, rect);
        self.content_per_mode(f, rect, mode);
        self.binds_per_mode(f, rect, mode);
    }
}

impl<'a> Menu<'a> {
    fn new(status: &'a Status, index: usize) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
        }
    }

    /// Hide the cursor if the current mode doesn't require one.
    /// Otherwise, display a cursor in the top row, at a correct column.
    ///
    /// # Errors
    ///
    /// may fail if we can't display on the terminal.
    fn cursor(&self, f: &mut Frame) -> Result<()> {
        let offset = self.tab.menu_mode.cursor_offset();
        let index = self.status.menu.input.index();
        let y = (offset + index) as u16;
        if self.tab.menu_mode.show_cursor() {
            f.set_cursor_position(Position::new(0, y));
        }
        Ok(())
    }

    fn menu_line(&self, f: &mut Frame, rect: &Rect) {
        let menu = MENU_STYLES.get().expect("Menu colors should be set").second;
        match self.tab.menu_mode {
            MenuMode::InputSimple(InputSimple::Chmod) => {
                let first = MENU_STYLES.get().expect("Menu colors should be set").first;
                self.menu_line_chmod(f, rect, first, menu);
            }
            edit => rect.print_with_attr(f, 1, 2, edit.second_line(), menu),
        };
    }

    fn menu_line_chmod(&self, f: &mut Frame, rect: &Rect, first: Style, menu: Style) -> usize {
        let mode_parsed = parse_input_mode(&self.status.menu.input.string());
        let mut col = 11;
        for (text, is_valid) in &mode_parsed {
            let attr = if *is_valid { first } else { menu };
            col += 1 + text.len();
            rect.print_with_attr(f, 1, col as u16, text, attr);
        }
        col
    }

    fn content_per_mode(&self, f: &mut Frame, rect: &Rect, mode: MenuMode) {
        match mode {
            MenuMode::Navigate(mode) => self.navigate(mode, f, rect),
            MenuMode::NeedConfirmation(mode) => self.confirm(mode, f, rect),
            MenuMode::InputCompleted(_) => self.completion(f, rect),
            MenuMode::InputSimple(mode) => Self::static_lines(mode.lines(), f, rect),
            _ => (),
        }
    }

    fn binds_per_mode(&self, f: &mut Frame, rect: &Rect, mode: MenuMode) {
        let height = rect.height;
        rect.clear_line(f, height - 1);
        rect.print_with_attr(
            f,
            height - 1,
            2,
            mode.binds_per_mode(),
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
    }

    fn static_lines(lines: &[&str], f: &mut Frame, rect: &Rect) {
        for (row, line, style) in enumerated_colored_iter!(lines) {
            Self::content_line(f, rect, row, line, style);
        }
    }

    fn navigate(&self, navigate: Navigate, f: &mut Frame, rect: &Rect) {
        if navigate.simple_draw_menu() {
            return self.status.menu.draw_navigate(f, rect, navigate);
        }
        match navigate {
            Navigate::Cloud => self.cloud(f, rect),
            Navigate::Context => self.context(f, rect),
            Navigate::Flagged => self.flagged(f, rect),
            Navigate::History => self.history(f, rect),
            Navigate::Picker => self.picker(f, rect),
            Navigate::Trash => self.trash(f, rect),
            _ => unreachable!("menu.simple_draw_menu should cover this mode"),
        }
    }

    fn history(&self, f: &mut Frame, rect: &Rect) {
        let selectable = &self.tab.history;
        let mut window = ContentWindow::new(selectable.len(), rect.height as usize);
        window.scroll_to(selectable.index);
        selectable.draw_menu(f, rect, &window)
    }

    fn trash(&self, f: &mut Frame, rect: &Rect) {
        let trash = &self.status.menu.trash;
        if trash.content().is_empty() {
            self.trash_is_empty(f, rect)
        } else {
            self.trash_content(f, rect, trash)
        };
    }

    fn trash_content(&self, f: &mut Frame, rect: &Rect, trash: &Trash) {
        let _ = trash.draw_menu(f, rect, &self.status.menu.window);
        let _ = rect.print_with_attr(
            f,
            1,
            2,
            &trash.help,
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
    }

    fn trash_is_empty(&self, f: &mut Frame, rect: &Rect) {
        let _ = Self::content_line(
            f,
            rect,
            0,
            "Trash is empty",
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
    }

    fn cloud(&self, f: &mut Frame, rect: &Rect) {
        let cloud = &self.status.menu.cloud;
        let mut desc = cloud.desc();
        if let Some((index, metadata)) = &cloud.metadata_repr {
            if index == &cloud.index {
                desc = format!("{desc} - {metadata}");
            }
        }
        let _ = rect.print_with_attr(
            f,
            2,
            2,
            &desc,
            MENU_STYLES
                .get()
                .expect("Menu colors should be set")
                .palette_4,
        );
        cloud.draw_menu(f, rect, &self.status.menu.window)
    }

    fn picker(&self, f: &mut Frame, rect: &Rect) {
        let selectable = &self.status.menu.picker;
        selectable.draw_menu(f, rect, &self.status.menu.window);
        if let Some(desc) = &selectable.desc {
            rect.clear_line(f, 1);
            rect.print_with_attr(
                f,
                1,
                2,
                desc,
                MENU_STYLES.get().expect("Menu colors should be set").second,
            );
        }
    }

    fn context(&self, f: &mut Frame, rect: &Rect) {
        self.context_selectable(f, rect);
        self.context_more_infos(f, rect)
    }

    fn context_selectable(&self, f: &mut Frame, rect: &Rect) {
        let selectable = &self.status.menu.context;
        rect.print_with_attr(
            f,
            1,
            2,
            "Pick an action.",
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
        let content = selectable.content();
        for (letter, (row, desc, attr)) in
            std::iter::zip(('a'..='z').cycle(), enumerated_colored_iter!(content))
        {
            let style = selectable.style(row, &attr);
            rect.print_with_attr(
                f,
                (row + 1 + ContentWindow::WINDOW_MARGIN_TOP) as u16,
                2,
                &format!("{letter} "),
                style,
            );
            Self::content_line(f, rect, row + 1, desc, style);
        }
    }

    fn context_more_infos(&self, f: &mut Frame, rect: &Rect) {
        let space_used = &self.status.menu.context.content.len();
        let more_info = MoreInfos::new(
            &self.tab.current_file().unwrap(),
            &self.status.internal_settings.opener,
        )
        .to_lines();
        for (row, text, style) in enumerated_colored_iter!(more_info) {
            Self::content_line(f, rect, space_used + row + 1, text, style);
        }
    }

    fn flagged(&self, f: &mut Frame, rect: &Rect) {
        self.flagged_files(f, rect);
        self.flagged_selected(f, rect);
    }

    fn flagged_files(&self, f: &mut Frame, rect: &Rect) {
        self.status
            .menu
            .flagged
            .draw_menu(f, rect, &self.status.menu.window);
    }

    fn flagged_selected(&self, f: &mut Frame, rect: &Rect) {
        if let Some(selected) = self.status.menu.flagged.selected() {
            let fileinfo = FileInfo::new(selected, &self.tab.users).unwrap();
            rect.print_with_attr(f, 2, 2, &fileinfo.format(6, 6).unwrap(), fileinfo.style());
        };
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&self, f: &mut Frame, rect: &Rect) {
        self.status
            .menu
            .completion
            .draw_menu(f, rect, &self.status.menu.window)
    }

    /// Display a list of edited (deleted, copied, moved, trashed) files for confirmation
    fn confirm(&self, confirmed_mode: NeedConfirmation, f: &mut Frame, rect: &Rect) {
        let dest = path_to_string(&self.tab.directory_of_selected().unwrap());

        Self::content_line(
            f,
            rect,
            0,
            &confirmed_mode.confirmation_string(&dest),
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
        match confirmed_mode {
            NeedConfirmation::EmptyTrash => self.confirm_empty_trash(f, rect),
            NeedConfirmation::BulkAction => self.confirm_bulk(f, rect),
            NeedConfirmation::DeleteCloud => self.confirm_delete_cloud(f, rect),
            _ => self.confirm_default(f, rect),
        };
    }

    fn confirm_default(&self, f: &mut Frame, rect: &Rect) {
        let content = &self.status.menu.flagged.content;
        for (row, path, attr) in enumerated_colored_iter!(content) {
            Self::content_line(
                f,
                rect,
                row + 2,
                path.to_str().context("Unreadable filename").unwrap(),
                attr,
            );
        }
    }

    fn confirm_bulk(&self, f: &mut Frame, rect: &Rect) {
        let content = self.status.menu.bulk.format_confirmation();
        for (row, line, style) in enumerated_colored_iter!(content) {
            Self::content_line(f, rect, row + 2, line, style);
        }
    }

    fn confirm_delete_cloud(&self, f: &mut Frame, rect: &Rect) {
        let line = if let Some(selected) = &self.status.menu.cloud.selected() {
            &format!(
                "{desc}{sel}",
                desc = self.status.menu.cloud.desc(),
                sel = selected.path()
            )
        } else {
            "No selected file"
        };
        Self::content_line(
            f,
            rect,
            3,
            line,
            MENU_STYLES
                .get()
                .context("MENU_ATTRS should be set")
                .unwrap()
                .palette_4,
        );
    }

    fn confirm_empty_trash(&self, f: &mut Frame, rect: &Rect) {
        if self.status.menu.trash.is_empty() {
            self.trash_is_empty(f, rect)
        } else {
            self.confirm_non_empty_trash(f, rect)
        }
    }

    fn confirm_non_empty_trash(&self, f: &mut Frame, rect: &Rect) {
        let content = self.status.menu.trash.content();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.trash.style(row, &attr);
            Self::content_line(f, rect, row + 4, &trashinfo.to_string(), attr);
        }
    }

    fn content_line(f: &mut Frame, rect: &Rect, row: usize, text: &str, style: Style) -> usize {
        rect.print_with_attr(
            f,
            (row + ContentWindow::WINDOW_MARGIN_TOP) as u16,
            4,
            text,
            style,
        );
        text.len()
    }
}

struct MenuFirstLine {
    content: Vec<String>,
}

impl Draw for MenuFirstLine {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        draw_colored_strings(f, 0, 1, &self.content, rect, false);
    }
}

impl MenuFirstLine {
    fn new(status: &Status) -> Self {
        Self {
            content: status.current_tab().menu_mode.line_display(status),
        }
    }
}

/// Is responsible for displaying content in the terminal.
/// It uses an already created terminal.
pub struct Display {
    /// The Tuikit terminal attached to the display.
    /// It will print every symbol shown on screen.
    term: Terminal<CrosstermBackend<Stdout>>,
}

impl Display {
    /// Returns a new `Display` instance from a terminal object.
    pub fn new(term: Terminal<CrosstermBackend<Stdout>>) -> Self {
        log_info!("starting display...");
        Self { term }
    }

    // TODO! render
    /// Display every possible content in the terminal.
    ///
    /// The top line
    ///
    /// The files if we're displaying them
    ///
    /// The cursor if a content is editable
    ///
    /// The help if `Mode::Help`
    ///
    /// The jump_list if `Mode::Jump`
    ///
    /// The completion list if any.
    ///
    /// The preview in preview mode.
    /// Displays one pane or two panes, depending of the width and current
    /// status of the application.
    pub fn display_all(&mut self, status: &MutexGuard<Status>) {
        self.hide_cursor();
        self.term.clear();
        let Ok(Size { width, height }) = self.term.size() else {
            return;
        };
        let rect = Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let width = self.term.size().unwrap().width;
        let wins = if status.display_settings.dual() && width > MIN_WIDTH_FOR_DUAL_PANE as u16 {
            self.dual_pane(status, rect)
        } else {
            self.single_pane(status, rect)
        };
        self.term.draw(|f| {});
    }

    /// Used to force a display of the cursor before leaving the application.
    /// Most of the times we don't need a cursor and it's hidden. We have to
    /// do it unless the shell won't display a cursor anymore.
    pub fn show_cursor(&mut self) -> Result<()> {
        Ok(self.term.show_cursor()?)
    }

    fn hide_cursor(&mut self) -> Result<()> {
        Ok(self.term.hide_cursor()?)
    }

    pub fn height(&self) -> Result<usize> {
        let height = self.term.size()?.height as usize;
        Ok(height)
    }

    fn size_for_menu_window(&self, tab: &Tab) -> Result<usize> {
        if tab.need_menu_window() {
            Ok(self.height()? / 2)
        } else {
            Ok(0)
        }
    }

    fn vertical_split<'a>(
        &self,
        files: &'a Files,
        menu: &'a Menu,
        file_border: Style,
        menu_border: Style,
        size: usize,
        parent_win: Rect,
    ) -> Vec<Rect> {
        let wins = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(parent_win)
            .to_vec();
        wins
    }

    /// Left File, Left Menu, Right File, Right Menu
    fn borders(&self, status: &Status) -> [Style; 4] {
        let menu_attrs = MENU_STYLES.get().expect("MENU_ATTRS should be set");
        let mut borders = [menu_attrs.inert_border; 4];
        let selected_border = menu_attrs.selected_border;
        borders[status.focus.index()] = selected_border;
        borders
    }

    // TODO: render
    fn dual_pane(&self, status: &Status, rect: Rect) -> Vec<Rect> {
        let width = rect.width as usize;
        let height = rect.height as usize;
        let (files_left, files_right) = FilesBuilder::dual(status, width);
        let menu_left = Menu::new(status, 0);
        let menu_right = Menu::new(status, 1);
        let borders = self.borders(status);
        let parent_wins = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(rect)
            .to_vec();
        let mut areas = self.vertical_split(
            &files_left,
            &menu_left,
            borders[0],
            borders[1],
            height,
            parent_wins[0],
        );
        areas.append(&mut self.vertical_split(
            &files_right,
            &menu_right,
            borders[2],
            borders[3],
            height,
            parent_wins[1],
        ));
        areas
    }

    // TODO: render
    fn single_pane(&self, status: &Status, rect: Rect) -> Vec<Rect> {
        let files_left = FilesBuilder::single(status);
        let menu_left = Menu::new(status, 0);
        let percent_left = self.size_for_menu_window(&status.tabs[0]).unwrap();
        let borders = self.borders(status);
        let areas = self.vertical_split(
            &files_left,
            &menu_left,
            borders[0],
            borders[1],
            percent_left,
            rect,
        );
        areas
    }
}

fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
    format!("{line_nr:0width$x}")
}

#[inline]
pub const fn color_to_style(color: Color) -> Style {
    Style {
        fg: Some(color),
        bg: None,
        add_modifier: Modifier::empty(),
        sub_modifier: Modifier::empty(),
        underline_color: None,
    }
}

fn draw_colored_strings(
    f: &mut Frame,
    row: usize,
    offset: usize,
    strings: &[String],
    rect: &Rect,
    effect_reverse: bool,
) {
    let mut col = 1;
    for (text, style) in std::iter::zip(
        strings.iter(),
        MENU_STYLES
            .get()
            .expect("Menu colors should be set")
            .palette()
            .iter()
            .cycle(),
    ) {
        let mut style = *style;
        if effect_reverse {
            style.add_modifier |= Modifier::REVERSED;
        }
        rect.print_with_attr(f, row as u16, (offset + col) as u16, text, style);
        col += text.len();
    }
}

fn draw_clickable_strings(
    f: &mut Frame,
    row: usize,
    offset: usize,
    elems: &[ClickableString],
    rect: &Rect,
    effect_reverse: bool,
) {
    for (elem, style) in std::iter::zip(
        elems.iter(),
        MENU_STYLES
            .get()
            .expect("Menu colors should be set")
            .palette()
            .iter()
            .cycle(),
    ) {
        let mut style = *style;
        if effect_reverse {
            style.add_modifier |= Modifier::REVERSED;
        }
        rect.print_with_attr(
            f,
            row as u16,
            (offset + elem.col()) as u16,
            elem.text(),
            style,
        );
    }
}

fn calc_line_row(i: usize, window: &ContentWindow) -> usize {
    i + ContentWindow::WINDOW_MARGIN_TOP - window.top
}
