use std::{cmp::min, io::Stdout, sync::MutexGuard};

use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Position, Rect, Size},
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::modes::{
    parse_input_mode, BinaryContent, Content, ContentWindow, Display as DisplayMode, FileInfo,
    HLContent, InputSimple, LineDisplay, Menu as MenuMode, MoreInfos, Navigate, NeedConfirmation,
    Preview, SecondLine, Selectable, TLine, Text, TextKind, Trash, Tree, Ueber, Window,
};
use crate::{
    app::{ClickableLine, ClickableString, Footer, Header, PreviewHeader, Status, Tab},
    modes::AnsiString,
};
use crate::{colored_skip_take, log_info};
use crate::{
    common::{path_to_string, UtfWidth},
    modes::FuzzyFinder,
};
use crate::{
    config::FILE_STYLES,
    io::{read_last_log_line, DrawMenu},
};
use crate::{
    config::{ColorG, Gradient, MENU_STYLES},
    modes::Input,
};

trait LimitWidth {
    fn limit_width(&self, width: usize) -> Self;
}

impl LimitWidth for &str {
    /// Limit a string slice to the give width, stopping at a char boundary.
    /// if the width is large enough, it's the width of the slice.
    fn limit_width(&self, width: usize) -> Self {
        let mut end = 0;
        let mut current_width = 0;

        for (index, grapheme) in self.grapheme_indices(true) {
            let grapheme_width = grapheme.chars().count();

            if current_width + grapheme_width > width {
                break;
            }
            current_width += grapheme_width;
            end = index + grapheme.len();
        }

        &self[..end]
    }
}

pub trait Canvas: Sized {
    fn print_with_style(&self, f: &mut Frame, row: u16, col: u16, content: &str, style: Style);

    fn print(&self, f: &mut Frame, row: u16, col: u16, content: &str);
}

impl Canvas for Rect {
    fn print_with_style(&self, f: &mut Frame, row: u16, col: u16, content: &str, style: Style) {
        // Define the area for the text
        let area = Rect {
            x: self.x + col,
            y: self.y + row,
            width: content.len() as u16,
            height: 1,
        };
        let available_width = self.width.saturating_sub(col) as usize;
        let displayed = content.limit_width(available_width);

        // Render at the specified coordinates
        f.render_widget(Span::styled(displayed, style), area);
    }

    fn print(&self, f: &mut Frame, row: u16, col: u16, content: &str) {
        self.print_with_style(
            f,
            row,
            col,
            content,
            Style {
                fg: Some(Color::Rgb(255, 255, 255)),
                ..Default::default()
            },
        )
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

/// Iter over the content, returning a triplet of `(index, line, style)`.
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
        .map(|((index, line), style)| (index, line, style))
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
            DisplayMode::Fuzzy => FuzzyDisplay::new(self).draw(f, rect),
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
        rect.print_with_style(
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
    ) {
        if status.menu.flagged.contains(path) {
            style.add_modifier |= Modifier::BOLD;
            rect.print_with_style(
                f,
                row as u16,
                0,
                "█",
                MENU_STYLES.get().expect("Menu colors should be set").second,
            );
        }
    }

    fn preview_in_right_tab(&self, f: &mut Frame, rect: &Rect) {
        let tab = &self.status.tabs[1];
        PreviewDisplay::new_with_args(self.status, tab, &self.attributes).draw(f, rect);
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

struct FuzzyDisplay<'a> {
    status: &'a Status,
}

impl<'a> Draw for FuzzyDisplay<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        self.fuzzy(f, rect)
    }
}

impl<'a> FuzzyDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
        }
    }

    fn fuzzy(&self, f: &mut Frame, rect: &Rect) {
        let Some(fuzzy) = &self.status.fuzzy else {
            return;
        };
        let rect = Rect {
            x: rect.x,
            y: rect.y + 2,
            width: rect.width,
            height: rect.height.saturating_sub(2),
        };
        let rects = Self::build_layout(rect);
        // Draw the match counts at the top
        let match_info = self.line_match_info(fuzzy);
        let match_count_paragraph = Self::paragraph_match_count(match_info);
        f.render_widget(match_count_paragraph, rects[0]);

        // Draw the matched items
        let items_paragraph = self.paragraph_matches(fuzzy);
        f.render_widget(items_paragraph, rects[2]);
        self.draw_prompt(fuzzy, f, rects[3]);
    }

    fn build_layout(area: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(area)
            .to_vec()
    }

    fn draw_prompt(&self, fuzzy: &FuzzyFinder<String>, f: &mut Frame, rect: Rect) {
        // Render the prompt string at the bottom
        let input = fuzzy.input.string();
        let prompt_paragraph = Paragraph::new(vec![Line::from(vec![
            Span::styled(
                "> ",
                MENU_STYLES
                    .get()
                    .expect("MENU_STYLES should be set")
                    .palette_3,
            ),
            Span::styled(
                input,
                MENU_STYLES
                    .get()
                    .expect("MENU_STYLES should be set")
                    .palette_2,
            ),
        ])])
        .block(Block::default().borders(Borders::NONE));

        // Render the prompt at the bottom of the layout
        f.render_widget(prompt_paragraph, rect);
        self.set_cursor_position(f, rect, &fuzzy.input);
    }

    fn set_cursor_position(&self, f: &mut Frame, rect: Rect, input: &Input) {
        // Move the cursor to the prompt
        f.set_cursor_position(Position {
            x: rect.x + input.index() as u16 + 2, // Adjust the cursor position for "> "
            y: rect.y,
        });
    }

    fn line_match_info(&self, fuzzy: &FuzzyFinder<String>) -> Line {
        Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", fuzzy.matched_item_count),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(" / ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", fuzzy.item_count),
                Style::default().fg(Color::Yellow),
            ),
        ])
    }

    fn paragraph_match_count(match_info: Line) -> Paragraph {
        Paragraph::new(match_info)
            .style(Style::default())
            .block(Block::default().borders(Borders::NONE))
    }

    fn paragraph_matches(&self, fuzzy: &FuzzyFinder<String>) -> Paragraph {
        let mut items: Vec<Line> = vec![];

        for (index, item) in fuzzy.content().iter().enumerate() {
            let item_spans = if index == fuzzy.index {
                Self::selected_line(item)
            } else {
                Self::non_selected_line(item)
            };
            items.push(item_spans);
        }
        Paragraph::new(items)
    }

    fn selected_line(render: &str) -> Line<'static> {
        Line::from(vec![Span::raw("> "), Span::raw(render.to_owned())])
            .black()
            .bg(MENU_STYLES
                .get()
                .expect("MENU_STYLES should be set")
                .palette_1
                .fg
                .unwrap())
    }

    fn non_selected_line(render: &str) -> Line<'static> {
        Line::from(vec![Span::raw("  "), Span::raw(render.to_owned())]).gray()
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
        FilesSecondLine::new(self.status, self.tab).draw(f, rect);
        self.files_content(f, rect);
        if !self.attributes.has_window_below && !self.attributes.is_right() {
            LogLine.draw(f, rect);
        }
    }

    fn files_content(&self, f: &mut Frame, rect: &Rect) {
        let group_owner_sizes = self.group_owner_size();
        let height = rect.height;
        for (index, file) in self.tab.dir_enum_skip_take() {
            self.files_line(f, rect, group_owner_sizes, index, file, height as usize);
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
        group_owner_sizes: (usize, usize),
        index: usize,
        file: &FileInfo,
        height: usize,
    ) {
        let row = index + ContentWindow::WINDOW_MARGIN_TOP - self.tab.window.top;
        if row + 2 > height {
            return;
        }
        let mut style = file.style();
        if index == self.tab.directory.index {
            style.add_modifier |= Modifier::REVERSED;
        }
        let content = Self::format_file_content(self.status, file, group_owner_sizes);
        Files::print_flagged_symbol(f, self.status, rect, row, &file.path, &mut style);
        let col = 1 + self.status.menu.flagged.contains(&file.path) as usize;
        rect.print_with_style(f, row as u16, col as u16, &content, style);
    }

    fn format_file_content(
        status: &Status,
        file: &FileInfo,
        owner_sizes: (usize, usize),
    ) -> String {
        if status.display_settings.metadata() {
            file.format(owner_sizes.1, owner_sizes.0).unwrap()
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
        FilesSecondLine::new(self.status, self.tab).draw(f, rect);
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
    ) {
        let (mut col, top, index, height) = position_param.export();
        let row = (index + ContentWindow::WINDOW_MARGIN_TOP).saturating_sub(top);
        if row + 2 > height {
            return;
        }

        let mut style = line_builder.style;
        let path = line_builder.path();
        Files::print_flagged_symbol(f, status, rect, row, path, &mut style);

        col += Self::tree_metadata(f, rect, with_medatadata, row, col, line_builder, style);
        col += if index == 0 { 2 } else { 1 };
        rect.print(f, row as u16, col as u16, line_builder.prefix());
        col += line_builder.prefix().utf_width();
        col += Self::tree_line_calc_flagged_offset(status, path);
        rect.print_with_style(f, row as u16, col as u16, &line_builder.filename(), style);
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
            let line = line_builder.metadata();
            let len = line.utf_width();
            rect.print_with_style(f, row as u16, col as u16, line, style);
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
        rect.print_with_style(
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
                token.print(f, rect, row_position, line_number_width);
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
            rect.print_with_style(
                f,
                row as u16,
                0,
                &format_line_nr_hex(i + 1 + window.top, line_number_width_hex),
                MENU_STYLES.get().expect("Menu colors should be set").first,
            );
            line.print_bytes(f, rect, row, line_number_width_hex + 1);
            line.print_ascii(f, rect, row, line_number_width_hex + 43);
        }
    }

    fn ueberzug(&self, image: &Ueber, rect: &Rect) {
        let width = rect.width;
        let height = rect.height;
        // image.match_index()?;
        image.draw(
            self.attributes.x_position as u16 + 2,
            3,
            width - 2,
            height - 2,
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
            if row + 2 > height as usize {
                break;
            }
            let offset = 3;
            for (i, (chr, style)) in AnsiString::parse(line).iter().enumerate() {
                rect.print_with_style(f, row as u16, (offset + i) as u16, &chr.to_string(), style)
            }
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
                rect.print_with_style(f, 1, 1, content, *style);
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
        let mut style = file.style();
        style.add_modifier ^= Modifier::REVERSED;

        Self {
            content: Some(file.format(owner_size, group_size).unwrap_or_default()),
            style: Some(style),
        }
    }
}

struct LogLine;

impl Draw for LogLine {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let height = rect.height;
        rect.print_with_style(
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
        rect.print_with_style(f, height - 1, 0, &" ".repeat(width as usize), background);
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
        if !self.tab.need_menu_window() {
            return;
        }
        let mode = self.tab.menu_mode;
        self.cursor(f, rect);
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
    fn cursor(&self, f: &mut Frame, rect: &Rect) {
        let offset = self.tab.menu_mode.cursor_offset();
        let index = self.status.menu.input.index();
        let x = rect.x + (offset + index) as u16;
        if self.tab.menu_mode.show_cursor() {
            f.set_cursor_position(Position::new(x, rect.y));
        }
    }

    fn menu_line(&self, f: &mut Frame, rect: &Rect) {
        let menu = MENU_STYLES.get().expect("Menu colors should be set").second;
        match self.tab.menu_mode {
            MenuMode::InputSimple(InputSimple::Chmod) => {
                let first = MENU_STYLES.get().expect("Menu colors should be set").first;
                self.menu_line_chmod(f, rect, first, menu);
            }
            edit => rect.print_with_style(f, 1, 2, edit.second_line(), menu),
        };
    }

    fn menu_line_chmod(&self, f: &mut Frame, rect: &Rect, first: Style, menu: Style) -> usize {
        let mode_parsed = parse_input_mode(&self.status.menu.input.string());
        let mut col = 11;
        for (text, is_valid) in &mode_parsed {
            let style = if *is_valid { first } else { menu };
            col += 1 + text.utf_width();
            rect.print_with_style(f, 1, col as u16, text, style);
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

        rect.clear_line(f, height.saturating_sub(2));
        rect.print_with_style(
            f,
            height.saturating_sub(2),
            2,
            mode.binds_per_mode(),
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
    }

    fn static_lines(lines: &[&str], f: &mut Frame, rect: &Rect) {
        for (row, line, style) in enumerated_colored_iter!(lines) {
            Self::content_line(f, rect, row as u16, line, style);
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
        trash.draw_menu(f, rect, &self.status.menu.window);
        rect.print_with_style(
            f,
            1,
            2,
            &trash.help,
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
    }

    fn trash_is_empty(&self, f: &mut Frame, rect: &Rect) {
        Self::content_line(
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
        rect.print_with_style(
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
            rect.print_with_style(
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
        rect.print_with_style(
            f,
            1,
            2,
            "Pick an action.",
            MENU_STYLES.get().expect("Menu colors should be set").second,
        );
        let content = selectable.content();
        let window = &self.status.menu.window;
        for (letter, (index, desc, style)) in std::iter::zip(
            ('a'..='z').cycle().skip(self.status.menu.window.top),
            colored_skip_take!(content, window),
        ) {
            let row = (index + 1 - window.top) as u16;
            if row + 2 + ContentWindow::WINDOW_MARGIN_TOP as u16 > rect.height {
                return;
            }
            let style = selectable.style(index, &style);
            rect.print_with_style(
                f,
                row + (ContentWindow::WINDOW_MARGIN_TOP) as u16,
                2,
                &format!("{letter} "),
                style,
            );
            Self::content_line(f, rect, row, desc, style);
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
            Self::content_line(f, rect, (space_used + row + 1) as u16, text, style);
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
            rect.print_with_style(f, 2, 2, &fileinfo.format(6, 6).unwrap(), fileinfo.style());
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
        for (row, path, style) in enumerated_colored_iter!(content) {
            Self::content_line(
                f,
                rect,
                row as u16 + 2,
                path.to_str().context("Unreadable filename").unwrap(),
                style,
            );
        }
    }

    fn confirm_bulk(&self, f: &mut Frame, rect: &Rect) {
        let content = self.status.menu.bulk.format_confirmation();
        for (row, line, style) in enumerated_colored_iter!(content) {
            Self::content_line(f, rect, row as u16 + 2, line, style);
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
                .context("MENU_STYLES should be set")
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
        for (row, trashinfo, style) in enumerated_colored_iter!(content) {
            let style = self.status.menu.trash.style(row, &style);
            Self::content_line(f, rect, row as u16 + 4, &trashinfo.to_string(), style);
        }
    }

    fn content_line(f: &mut Frame, rect: &Rect, row: u16, text: &str, style: Style) -> usize {
        rect.print_with_style(
            f,
            row + ContentWindow::WINDOW_MARGIN_TOP as u16,
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
        if status.should_be_cleared() {
            self.term.clear().unwrap();
        }
        let Ok(Size { width, height }) = self.term.size() else {
            return;
        };
        let rect = Rect::new(0, 0, width, height);
        let inside_border_rect = Rect::new(1, 1, width.saturating_sub(2), height.saturating_sub(2));
        let borders = self.borders(status);
        if status.display_settings.dual() && width > MIN_WIDTH_FOR_DUAL_PANE as u16 {
            self.draw_dual(rect, inside_border_rect, borders, status);
        } else {
            self.draw_single(rect, inside_border_rect, borders, status);
        };
    }

    fn draw_dual(
        &mut self,
        rect: Rect,
        inside_border_rect: Rect,
        borders: [Style; 4],
        status: &Status,
    ) {
        let (file_left, file_right) = FilesBuilder::dual(status, rect.width as usize);
        let menu_left = Menu::new(status, 0);
        let menu_right = Menu::new(status, 1);
        let parent_wins = self.horizontal_split(rect);
        let have_menu_left = status.tabs[0].need_menu_window();
        let have_menu_right = status.tabs[1].need_menu_window();
        let bordered_wins = self.dual_bordered_rect(parent_wins, have_menu_left, have_menu_right);
        let inside_wins =
            self.dual_inside_rect(inside_border_rect, have_menu_left, have_menu_right);
        self.render_dual(
            borders,
            bordered_wins,
            inside_wins,
            (file_left, file_right),
            (menu_left, menu_right),
        );
    }

    fn render_dual(
        &mut self,
        borders: [Style; 4],
        bordered_wins: Vec<Rect>,
        inside_wins: Vec<Rect>,
        files: (Files, Files),
        menus: (Menu, Menu),
    ) {
        self.term
            .draw(|f| {
                // 0 2
                // 1 3
                Self::draw_dual_borders(borders, f, &bordered_wins);
                files.0.draw(f, &inside_wins[0]);
                menus.0.draw(f, &inside_wins[1]);
                files.1.draw(f, &inside_wins[2]);
                menus.1.draw(f, &inside_wins[3]);
            })
            .unwrap();
    }

    fn draw_single(
        &mut self,
        rect: Rect,
        inside_border_rect: Rect,
        borders: [Style; 4],
        status: &Status,
    ) {
        let file_left = FilesBuilder::single(status);
        let menu_left = Menu::new(status, 0);
        let need_menu = status.tabs[0].need_menu_window();
        let bordered_wins = self.vertical_split_border(rect, need_menu);
        let inside_wins = self.vertical_split_inner(inside_border_rect, need_menu);
        self.render_single(borders, bordered_wins, inside_wins, file_left, menu_left)
    }

    fn render_single(
        &mut self,
        borders: [Style; 4],
        bordered_wins: Vec<Rect>,
        inside_wins: Vec<Rect>,
        file_left: Files,
        menu_left: Menu,
    ) {
        self.term
            .draw(|f| {
                Self::draw_single_borders(borders, f, &bordered_wins);
                file_left.draw(f, &inside_wins[0]);
                menu_left.draw(f, &inside_wins[1]);
            })
            .unwrap();
    }

    fn draw_n_borders(n: usize, borders: [Style; 4], f: &mut Frame, wins: &[Rect]) {
        for i in 0..n {
            let bordered_block = Block::default().borders(Borders::ALL).style(borders[i]);
            f.render_widget(bordered_block, wins[i]);
        }
    }

    fn draw_dual_borders(borders: [Style; 4], f: &mut Frame, wins: &[Rect]) {
        Self::draw_n_borders(4, borders, f, wins)
    }

    fn draw_single_borders(borders: [Style; 4], f: &mut Frame, wins: &[Rect]) {
        Self::draw_n_borders(2, borders, f, wins)
    }

    /// Used to force a display of the cursor before leaving the application.
    /// Most of the times we don't need a cursor and it's hidden. We have to
    /// do it unless the shell won't display a cursor anymore.
    pub fn show_cursor(&mut self) -> Result<()> {
        Ok(self.term.show_cursor()?)
    }

    pub fn height(&self) -> Result<usize> {
        let height = self.term.size()?.height as usize;
        Ok(height)
    }

    fn horizontal_split(&self, rect: Rect) -> Vec<Rect> {
        let left_rect = Rect::new(rect.x, rect.y, rect.width / 2, rect.height);
        let right_rect = Rect::new(rect.x + rect.width / 2, rect.y, rect.width / 2, rect.height);
        vec![left_rect, right_rect]
    }

    fn dual_bordered_rect(
        &self,
        parent_wins: Vec<Rect>,
        have_menu_left: bool,
        have_menu_right: bool,
    ) -> Vec<Rect> {
        let mut bordered_wins = self.vertical_split_border(parent_wins[0], have_menu_left);
        bordered_wins.append(&mut self.vertical_split_border(parent_wins[1], have_menu_right));
        bordered_wins
    }

    // TODO: do the horizontal split by hand
    fn vertical_split_inner(&self, parent_win: Rect, have_menu: bool) -> Vec<Rect> {
        let (top, bot) = if have_menu {
            (parent_win.height / 2 - 1, parent_win.height / 2)
        } else {
            (parent_win.height, 0)
        };
        let top_rect = Rect::new(parent_win.x, parent_win.y, parent_win.width, top);
        let bot_rect = Rect::new(parent_win.x, parent_win.y + top + 2, parent_win.width, bot);
        vec![top_rect, bot_rect]
    }

    fn vertical_split_border(&self, parent_win: Rect, have_menu: bool) -> Vec<Rect> {
        let (top, bot) = if have_menu {
            (parent_win.height / 2, parent_win.height / 2)
        } else {
            (parent_win.height, 0)
        };
        let top_rect = Rect::new(parent_win.x, parent_win.y, parent_win.width, top);
        let bot_rect = Rect::new(parent_win.x, parent_win.y + top, parent_win.width, bot);
        vec![top_rect, bot_rect]
    }

    /// Left File, Left Menu, Right File, Right Menu
    fn borders(&self, status: &Status) -> [Style; 4] {
        let menu_styles = MENU_STYLES.get().expect("MENU_STYLES should be set");
        let mut borders = [menu_styles.inert_border; 4];
        let selected_border = menu_styles.selected_border;
        borders[status.focus.index()] = selected_border;
        borders
    }

    fn dual_inside_rect(
        &self,
        rect: Rect,
        have_menu_left: bool,
        have_menu_right: bool,
    ) -> Vec<Rect> {
        let parent_wins = {
            let left_rect = Rect::new(rect.x, rect.y, rect.width / 2 - 1, rect.height);
            let right_rect = Rect::new(
                rect.x + rect.width / 2 + 1,
                rect.y,
                rect.width / 2 - 2,
                rect.height,
            );
            vec![left_rect, right_rect]
        };
        let mut areas = self.vertical_split_inner(parent_wins[0], have_menu_left);
        areas.append(&mut self.vertical_split_inner(parent_wins[1], have_menu_right));
        areas
    }

    pub fn restore_terminal(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.term.backend_mut(), LeaveAlternateScreen)?;
        self.term.show_cursor()?;
        Ok(())
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
        rect.print_with_style(f, row as u16, (offset + col) as u16, text, style);
        col += text.utf_width();
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
        rect.print_with_style(
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
