use std::{
    io::{self, Stdout, Write},
    rc::Rc,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use nucleo::Config;
use parking_lot::MutexGuard;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Offset, Position, Rect, Size},
    prelude::*,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};

use crate::common::path_to_string;
use crate::config::{with_icon, with_icon_metadata, ColorG, Gradient, MATCHER, MENU_STYLES};
use crate::io::{read_last_log_line, DrawMenu};
use crate::log_info;
use crate::modes::{
    highlighted_text, parse_input_permission, AnsiString, BinLine, BinaryContent, Content,
    ContentWindow, Display as DisplayMode, FileInfo, FuzzyFinder, HLContent, Input, InputSimple,
    LineDisplay, Menu as MenuMode, MoreInfos, Navigate, NeedConfirmation, Preview, Remote,
    SecondLine, Selectable, TLine, TakeSkip, TakeSkipEnum, Text, TextKind, Trash, Tree, Ueber,
};
use crate::{
    app::{ClickableLine, Footer, Header, PreviewHeader, Status, Tab},
    io::{Scalers, UeConf, Ueberzug},
};

pub trait Offseted {
    fn offseted(&self, x: u16, y: u16) -> Self;
}

impl Offseted for Rect {
    /// Returns a new rect moved by x horizontally and y vertically and constrained to the current rect.
    /// It won't draw outside of the original rect since it it also intersected by the original rect.
    fn offseted(&self, x: u16, y: u16) -> Self {
        self.offset(Offset {
            x: x as i32,
            y: y as i32,
        })
        .intersection(*self)
    }
}

/// Common trait all "window" should implement.
/// It's mostly used as an entry point for the rendering and should call another method.
trait Draw {
    /// Entry point for window rendering.
    fn draw(&self, f: &mut Frame, rect: &Rect);
}

macro_rules! colored_iter {
    ($t:ident) => {
        std::iter::zip(
            $t.iter(),
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
    };
}

/// At least 120 chars width to display 2 tabs.
pub const MIN_WIDTH_FOR_DUAL_PANE: u16 = 120;

enum TabPosition {
    Left,
    Right,
}

/// Bunch of attributes describing the state of a main window
/// relatively to other windows
struct FilesAttributes {
    /// horizontal position, in cells
    x_position: u16,
    /// is this the left or right window ?
    tab_position: TabPosition,
    /// is this tab selected ?
    is_selected: bool,
    /// is there a menuary window ?
    has_window_below: bool,
}

impl FilesAttributes {
    fn new(
        x_position: u16,
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
    fn dual(status: &Status, width: u16) -> (Files, Files) {
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

impl<'a> Files<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect, ueberzug: Arc<Mutex<Ueberzug>>) {
        let use_log_line = self.use_log_line();
        let rects = Rects::files(rect, use_log_line);

        if self.should_preview_in_right_tab() {
            self.preview_in_right_tab(f, &rects[0], &rects[2], ueberzug);
            return;
        }

        self.header(f, &rects[0]);
        self.copy_progress_bar(f, &rects[1]);
        self.second_line(f, &rects[1]);
        self.content(f, &rects[1], &rects[2], ueberzug);
        if use_log_line {
            self.log_line(f, &rects[3]);
        }
        self.footer(f, rects.last().expect("Shouldn't be empty"));
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

    fn use_log_line(&self) -> bool {
        matches!(
            self.tab.display_mode,
            DisplayMode::Directory | DisplayMode::Tree
        ) && !self.attributes.has_window_below
            && !self.attributes.is_right()
    }

    fn should_preview_in_right_tab(&self) -> bool {
        self.status.session.dual() && self.is_right() && self.status.session.preview()
    }

    fn preview_in_right_tab(
        &self,
        f: &mut Frame,
        header_rect: &Rect,
        content_rect: &Rect,
        ueberzug: Arc<Mutex<Ueberzug>>,
    ) {
        let tab = &self.status.tabs[1];
        PreviewHeader::into_default_preview(self.status, tab, content_rect.width).draw_left(
            f,
            *header_rect,
            self.status.index == 1,
        );
        PreviewDisplay::new_with_args(self.status, tab, &self.attributes).draw(
            f,
            content_rect,
            ueberzug,
        );
    }

    fn is_right(&self) -> bool {
        self.attributes.is_right()
    }

    fn header(&self, f: &mut Frame, rect: &Rect) {
        FilesHeader::new(self.status, self.tab, self.attributes.is_selected).draw(f, rect);
    }

    /// Display a copy progress bar on the left tab.
    /// Nothing is drawn if there's no copy atm.
    /// If the copy file queue has length > 1, we also display its size.
    fn copy_progress_bar(&self, f: &mut Frame, rect: &Rect) {
        if self.is_right() {
            return;
        }
        CopyProgressBar::new(self.status).draw(f, rect);
    }

    fn second_line(&self, f: &mut Frame, rect: &Rect) {
        if matches!(
            self.tab.display_mode,
            DisplayMode::Directory | DisplayMode::Tree
        ) {
            FilesSecondLine::new(self.status, self.tab).draw(f, rect);
        }
    }

    fn content(
        &self,
        f: &mut Frame,
        second_line_rect: &Rect,
        content_rect: &Rect,
        ueberzug: Arc<Mutex<Ueberzug>>,
    ) {
        match &self.tab.display_mode {
            DisplayMode::Directory => DirectoryDisplay::new(self).draw(f, content_rect),
            DisplayMode::Tree => TreeDisplay::new(self).draw(f, content_rect),
            DisplayMode::Preview => PreviewDisplay::new(self).draw(f, content_rect, ueberzug),
            DisplayMode::Fuzzy => FuzzyDisplay::new(self).fuzzy(f, second_line_rect, content_rect),
        }
    }

    fn log_line(&self, f: &mut Frame, rect: &Rect) {
        LogLine.draw(f, rect);
    }

    fn footer(&self, f: &mut Frame, rect: &Rect) {
        FilesFooter::new(self.status, self.tab, self.attributes.is_selected).draw(f, rect);
    }
}

struct CopyProgressBar<'a> {
    status: &'a Status,
}

impl<'a> Draw for CopyProgressBar<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let Some(content) = self.status.internal_settings.format_copy_progress() else {
            return;
        };
        let p_rect = rect.offseted(1, 0);
        Span::styled(
            &content,
            MENU_STYLES
                .get()
                .expect("Menu colors should be set")
                .palette_2,
        )
        .render(p_rect, f.buffer_mut());
    }
}

impl<'a> CopyProgressBar<'a> {
    fn new(status: &'a Status) -> Self {
        Self { status }
    }
}

struct FuzzyDisplay<'a> {
    status: &'a Status,
}

impl<'a> FuzzyDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
        }
    }

    fn fuzzy(&self, f: &mut Frame, second_line_rect: &Rect, content_rect: &Rect) {
        let Some(fuzzy) = &self.status.fuzzy else {
            return;
        };
        let rects = Rects::fuzzy(content_rect);

        self.draw_prompt(fuzzy, f, second_line_rect);
        self.draw_match_counts(fuzzy, f, &rects[0]);
        self.draw_matches(fuzzy, f, rects[1]);
    }

    /// Draw the matched items
    fn draw_match_counts(&self, fuzzy: &FuzzyFinder<String>, f: &mut Frame, rect: &Rect) {
        let match_info = self.line_match_info(fuzzy);
        let match_count_paragraph = Self::paragraph_match_count(match_info);
        f.render_widget(match_count_paragraph, *rect);
    }

    fn draw_prompt(&self, fuzzy: &FuzzyFinder<String>, f: &mut Frame, rect: &Rect) {
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

        f.render_widget(prompt_paragraph, *rect);
        self.set_cursor_position(f, rect, &fuzzy.input);
    }

    fn set_cursor_position(&self, f: &mut Frame, rect: &Rect, input: &Input) {
        // Move the cursor to the prompt
        f.set_cursor_position(Position {
            x: rect.x + input.index() as u16 + 2,
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
            Span::raw(" "),
        ])
    }

    fn paragraph_match_count(match_info: Line) -> Paragraph {
        Paragraph::new(match_info)
            .style(Style::default())
            .right_aligned()
            .block(Block::default().borders(Borders::NONE))
    }

    fn draw_matches(&self, fuzzy: &FuzzyFinder<String>, f: &mut Frame, rect: Rect) {
        let snapshot = fuzzy.matcher.snapshot();
        let (top, bottom) = fuzzy.top_bottom();
        let mut indices = vec![];
        let mut matcher = MATCHER.lock();
        matcher.config = Config::DEFAULT;
        let is_file = fuzzy.kind.is_file();
        if is_file {
            matcher.config.set_match_paths();
        }
        snapshot
            .matched_items(top..bottom)
            .enumerate()
            .for_each(|(index, t)| {
                snapshot.pattern().column_pattern(0).indices(
                    t.matcher_columns[0].slice(..),
                    &mut matcher,
                    &mut indices,
                );
                let text = t.matcher_columns[0].to_string();
                let highlights_usize = Self::highlights_indices(&mut indices);
                let line = highlighted_text(
                    &text,
                    &highlights_usize,
                    index as u32 + top == fuzzy.index,
                    is_file,
                );
                let line_rect = Self::line_rect(rect, index);
                line.render(line_rect, f.buffer_mut());
            });
    }

    fn highlights_indices(indices: &mut Vec<u32>) -> Vec<usize> {
        indices.sort_unstable();
        indices.dedup();
        let highlights = indices.drain(..);
        highlights.map(|index| index as usize).collect()
    }

    fn line_rect(rect: Rect, index: usize) -> Rect {
        let mut line_rect = rect;
        line_rect.y += index as u16;
        line_rect
    }
}

struct DirectoryDisplay<'a> {
    status: &'a Status,
    tab: &'a Tab,
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
        let group_owner_sizes = self.group_owner_size();
        let p_rect = rect.offseted(2, 0);
        let formater = self.pick_formater();
        let lines: Vec<_> = self
            .tab
            .dir_enum_skip_take()
            .map(|(index, file)| self.files_line(group_owner_sizes, index, file, &formater))
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }

    fn pick_formater(&self) -> fn(&FileInfo, (usize, usize)) -> String {
        let with_metadata = self.status.session.metadata();
        let with_icon = with_icon();
        let with_icon_metadata = with_icon_metadata();
        if with_metadata && with_icon_metadata {
            Self::format_file_metadata_icon
        } else if with_metadata {
            Self::format_file_metadata
        } else if with_icon {
            Self::format_file_simple_icon
        } else {
            Self::format_file_simple
        }
    }

    fn group_owner_size(&self) -> (usize, usize) {
        if self.status.session.metadata() {
            (
                self.tab.directory.group_column_width(),
                self.tab.directory.owner_column_width(),
            )
        } else {
            (0, 0)
        }
    }

    fn files_line<'b>(
        &self,
        group_owner_sizes: (usize, usize),
        index: usize,
        file: &FileInfo,
        formater: &fn(&FileInfo, (usize, usize)) -> String,
    ) -> Line<'b> {
        let mut style = file.style();
        self.reverse_selected(index, &mut style);
        let content = formater(file, group_owner_sizes);
        Line::from(vec![
            self.span_flagged_symbol(file, &mut style),
            Span::styled(content, style),
        ])
    }

    fn reverse_selected(&self, index: usize, style: &mut Style) {
        if index == self.tab.directory.index {
            style.add_modifier |= Modifier::REVERSED;
        }
    }

    fn span_flagged_symbol<'b>(&self, file: &FileInfo, style: &mut Style) -> Span<'b> {
        if self.status.menu.flagged.contains(&file.path) {
            style.add_modifier |= Modifier::BOLD;
            Span::styled(
                "█",
                MENU_STYLES.get().expect("Menu colors should be set").second,
            )
        } else {
            Span::raw("")
        }
    }

    fn format_file_metadata(file: &FileInfo, owner_sizes: (usize, usize)) -> String {
        file.format_metadata(owner_sizes.1, owner_sizes.0).unwrap()
    }

    fn format_file_metadata_icon(file: &FileInfo, owner_sizes: (usize, usize)) -> String {
        file.format_metadata_icon(owner_sizes.1, owner_sizes.0)
            .unwrap()
    }

    fn format_file_simple(file: &FileInfo, _owner_sizes: (usize, usize)) -> String {
        file.format_simple().unwrap()
    }

    fn format_file_simple_icon(file: &FileInfo, _owner_sizes: (usize, usize)) -> String {
        file.format_simple_icon().unwrap()
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
        Self::tree_content(
            self.status,
            &self.tab.tree,
            &self.tab.window,
            self.status.session.metadata(),
            f,
            rect,
        )
    }

    fn tree_content(
        status: &Status,
        tree: &Tree,
        window: &ContentWindow,
        with_metadata: bool,
        f: &mut Frame,
        rect: &Rect,
    ) {
        let p_rect = rect.offseted(1, 0);
        let with_icon = Self::use_icon(with_metadata);
        let lines: Vec<_> = tree
            .lines_enum_skip_take(window)
            .map(|(index, line_builder)| {
                Self::tree_line(status, index == 0, line_builder, with_metadata, with_icon)
            })
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }

    fn use_icon(with_metadata: bool) -> bool {
        (!with_metadata && with_icon()) || with_icon_metadata()
    }

    fn tree_line<'b>(
        status: &Status,
        with_offset: bool,
        line_builder: &'b TLine,
        with_medatadata: bool,
        with_icon: bool,
    ) -> Line<'b> {
        let mut style = line_builder.style;
        let path = line_builder.path();
        Line::from(vec![
            Self::span_flagged_symbol(status, path, &mut style),
            Self::tree_metadata_line(with_medatadata, line_builder, style),
            Span::raw(line_builder.prefix()),
            Span::raw(" ".repeat(Self::tree_line_calc_flagged_offset_line(status, path))),
            Span::raw(" ".repeat(with_offset as usize)),
            Span::styled(line_builder.filename(with_icon), style),
        ])
    }

    fn span_flagged_symbol<'b>(
        status: &Status,
        path: &std::path::Path,
        style: &mut Style,
    ) -> Span<'b> {
        if status.menu.flagged.contains(path) {
            style.add_modifier |= Modifier::BOLD;
            Span::styled(
                "█",
                MENU_STYLES.get().expect("Menu colors should be set").second,
            )
        } else {
            Span::raw(" ")
        }
    }

    fn tree_metadata_line(with_medatadata: bool, line_builder: &TLine, style: Style) -> Span {
        if with_medatadata {
            let line = line_builder.metadata();
            Span::styled(line, style)
        } else {
            Span::raw("")
        }
    }

    fn tree_line_calc_flagged_offset_line(status: &Status, path: &std::path::Path) -> usize {
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
impl<'a> PreviewDisplay<'a> {
    fn draw(&self, f: &mut Frame, rect: &Rect, ueberzug: Arc<Mutex<Ueberzug>>) {
        self.preview(f, rect, ueberzug)
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

    fn preview(&self, f: &mut Frame, rect: &Rect, ueberzug: Arc<Mutex<Ueberzug>>) {
        let tab = self.tab;
        let window = &tab.window;
        let length = tab.preview.len();
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                let number_col_width = Self::number_width(length);
                self.syntaxed(f, syntaxed, length, rect, number_col_width, window)
            }
            Preview::Binary(bin) => self.binary(f, bin, length, rect, window),
            Preview::Ueberzug(image) => self.ueberzug(image, rect, ueberzug),
            Preview::Tree(tree_preview) => self.tree_preview(f, tree_preview, window, rect),
            Preview::Text(ansi_text) if matches!(ansi_text.kind, TextKind::CommandStdout) => {
                self.ansi_text(f, ansi_text, length, rect, window)
            }
            Preview::Text(text) => self.normal_text(f, text, length, rect, window),

            Preview::Empty => (),
        };
    }

    fn line_number_span<'b>(
        line_number_to_print: &usize,
        number_col_width: usize,
        style: Style,
    ) -> Span<'b> {
        Span::styled(
            format!("{line_number_to_print:>number_col_width$}  "),
            style,
        )
    }

    /// Number of digits in decimal representation
    fn number_width(mut number: usize) -> usize {
        let mut width = 0;
        while number != 0 {
            width += 1;
            number /= 10;
        }
        width
    }

    /// Draw every line of the text
    fn normal_text(
        &self,
        f: &mut Frame,
        text: &Text,
        length: usize,
        rect: &Rect,
        window: &ContentWindow,
    ) {
        let p_rect = rect.offseted(2, 0);
        let lines: Vec<_> = text
            .take_skip(window.top, window.bottom, length)
            .map(Line::raw)
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }

    fn syntaxed(
        &self,
        f: &mut Frame,
        syntaxed: &HLContent,
        length: usize,
        rect: &Rect,
        number_col_width: usize,
        window: &ContentWindow,
    ) {
        let p_rect = rect.offseted(3, 0);
        let number_col_style = MENU_STYLES.get().expect("").first;
        let lines: Vec<_> = syntaxed
            .take_skip_enum(window.top, window.bottom, length)
            .map(|(index, vec_line)| {
                let mut line = vec![Self::line_number_span(
                    &index,
                    number_col_width,
                    number_col_style,
                )];
                line.append(
                    &mut vec_line
                        .iter()
                        .map(|token| Span::styled(&token.content, token.style))
                        .collect::<Vec<_>>(),
                );
                Line::from(line)
            })
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }

    fn binary(
        &self,
        f: &mut Frame,
        bin: &BinaryContent,
        length: usize,
        rect: &Rect,
        window: &ContentWindow,
    ) {
        let p_rect = rect.offseted(3, 0);
        let line_number_width_hex = bin.number_width_hex();
        let (style_number, style_ascii) = {
            let ms = MENU_STYLES.get().expect("Menu colors should be set");
            (ms.first, ms.second)
        };
        let lines: Vec<_> = (*bin)
            .take_skip_enum(window.top, window.bottom, length)
            .map(|(index, bin_line)| {
                Line::from(vec![
                    Span::styled(
                        BinLine::format_line_nr_hex(index + 1 + window.top, line_number_width_hex),
                        style_number,
                    ),
                    Span::raw(bin_line.format_hex()),
                    Span::raw(" "),
                    Span::styled(bin_line.format_as_ascii(), style_ascii),
                ])
            })
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }

    /// Draw the image with ueberzug in the current window.
    /// The position is absolute, which is problematic when the app is embeded into a floating terminal.
    fn ueberzug(&self, image: &Ueber, rect: &Rect, ueberzug: Arc<Mutex<Ueberzug>>) {
        let identifier = &image.identifier;
        let path = &image.images[image.image_index()].to_string_lossy();
        let x = self.attributes.x_position + 1;
        let y = 2;
        let width = Some(rect.width);
        let height = Some(rect.height.saturating_sub(1));
        let scaler = Some(Scalers::FitContain);
        let config = &UeConf {
            identifier,
            path,
            x,
            y,
            width,
            height,
            scaler,
            ..Default::default()
        };

        if let Err(e) = ueberzug
            .lock()
            .expect("Couldn't lock ueberzug")
            .draw(config)
        {
            log_info!(
                "Ueberzug could not draw {}, from path {}.\n{e}",
                image.identifier,
                path
            );
        };
    }

    fn tree_preview(&self, f: &mut Frame, tree: &Tree, window: &ContentWindow, rect: &Rect) {
        TreeDisplay::tree_content(self.status, tree, window, false, f, rect)
    }

    fn ansi_text(
        &self,
        f: &mut Frame,
        ansi_text: &Text,
        length: usize,
        rect: &Rect,
        window: &ContentWindow,
    ) {
        let p_rect = rect.offseted(3, 0);
        let lines: Vec<_> = ansi_text
            .take_skip(window.top, window.bottom, length)
            .map(|line| {
                Line::from(
                    AnsiString::parse(line)
                        .iter()
                        .map(|(chr, style)| Span::styled(chr.to_string(), style))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
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
    /// something else.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let width = rect.width;
        let header: Box<dyn ClickableLine> = match self.tab.display_mode {
            DisplayMode::Preview => Box::new(PreviewHeader::new(self.status, self.tab, width)),
            _ => Box::new(Header::new(self.status, self.tab).expect("Couldn't build header")),
        };
        header.draw_left(f, *rect, self.is_selected);
        header.draw_right(f, *rect, self.is_selected);
    }
}

impl<'a> FilesHeader<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Self {
        Self {
            status,
            tab,
            is_selected,
        }
    }
}

#[derive(Default)]
struct FilesSecondLine {
    content: Option<String>,
    style: Option<Style>,
}

impl Draw for FilesSecondLine {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let p_rect = rect.offseted(1, 0);
        if let (Some(content), Some(style)) = (&self.content, &self.style) {
            Span::styled(content, *style).render(p_rect, f.buffer_mut());
        };
    }
}

impl FilesSecondLine {
    fn new(status: &Status, tab: &Tab) -> Self {
        if tab.display_mode.is_preview() || status.session.metadata() {
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
            content: Some(
                file.format_metadata(owner_size, group_size)
                    .unwrap_or_default(),
            ),
            style: Some(style),
        }
    }
}

struct LogLine;

impl Draw for LogLine {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let p_rect = rect.offseted(4, 0);
        let log = &read_last_log_line();
        Span::styled(
            log,
            MENU_STYLES.get().expect("Menu colors should be set").second,
        )
        .render(p_rect, f.buffer_mut());
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
        match self.tab.display_mode {
            DisplayMode::Preview => (),
            _ => {
                let Ok(footer) = Footer::new(self.status, self.tab) else {
                    return;
                };
                // let p_rect = rect.offseted(0, rect.height.saturating_sub(1));
                footer.draw_left(f, *rect, self.is_selected);
            }
        }
    }
}

impl<'a> FilesFooter<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Self {
        Self {
            status,
            tab,
            is_selected,
        }
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

    /// Render a generic content of elements which are references to str.
    /// It creates a new rect, offseted by `x, y` and intersected with rect.
    /// Each element of content is wraped by a styled span (with his own style) and then wrapped by a line.
    /// The iteration only take enough element to be displayed in the rect.
    /// Then we create a paragraph with default parameters and render it.
    fn render_content<T>(content: &[T], f: &mut Frame, rect: &Rect, x: u16, y: u16)
    where
        T: AsRef<str>,
    {
        let p_rect = rect.offseted(x, y);
        let lines: Vec<_> = colored_iter!(content)
            .map(|(text, style)| Line::from(vec![Span::styled(text.as_ref(), style)]))
            .take(p_rect.height as usize + 2)
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }

    /// Hide the cursor if the current mode doesn't require one.
    /// Otherwise, display a cursor in the top row, at a correct column.
    ///
    /// # Errors
    ///
    /// may fail if we can't display on the terminal.
    fn cursor(&self, f: &mut Frame, rect: &Rect) {
        let offset = self.tab.menu_mode.cursor_offset();
        let index = self.status.menu.input.index() as u16;
        let x = rect.x + offset + index;
        if self.tab.menu_mode.show_cursor() {
            f.set_cursor_position(Position::new(x, rect.y));
        }
    }

    fn menu_line(&self, f: &mut Frame, rect: &Rect) {
        let menu_style = MENU_STYLES.get().expect("Menu colors should be set");
        let menu = menu_style.second;
        match self.tab.menu_mode {
            MenuMode::InputSimple(InputSimple::Chmod) => {
                let first = menu_style.first;
                self.menu_line_chmod(f, rect, first, menu);
            }
            MenuMode::InputSimple(InputSimple::Remote) => {
                let palette_3 = menu_style.palette_3;
                self.menu_line_remote(f, rect, palette_3);
            }
            edit => {
                let rect = rect.offseted(2, 1);
                Span::styled(edit.second_line(), menu).render(rect, f.buffer_mut());
            }
        };
    }

    fn menu_line_chmod(&self, f: &mut Frame, rect: &Rect, first: Style, menu: Style) {
        let input = self.status.menu.input.string();
        let spans: Vec<_> = parse_input_permission(&input)
            .iter()
            .map(|(text, is_valid)| {
                let style = if *is_valid { first } else { menu };
                Span::styled(*text, style)
            })
            .collect();
        let p_rect = rect.offseted(11, 1);
        Line::from(spans).render(p_rect, f.buffer_mut());
    }

    fn menu_line_remote(&self, f: &mut Frame, rect: &Rect, first: Style) {
        let input = self.status.menu.input.string();
        let current_path = path_to_string(&self.tab.current_path());

        if let Some(remote) = Remote::from_input(input, &current_path) {
            let command = format!("{command:?}", command = remote.command());
            let p_rect = rect.offseted(4, 8);
            Line::styled(command, first).render(p_rect, f.buffer_mut());
        };
    }

    fn content_per_mode(&self, f: &mut Frame, rect: &Rect, mode: MenuMode) {
        match mode {
            MenuMode::Navigate(mode) => self.navigate(mode, f, rect),
            MenuMode::NeedConfirmation(mode) => self.confirm(mode, f, rect),
            MenuMode::InputCompleted(_) => self.completion(f, rect),
            MenuMode::InputSimple(mode) => Self::input_simple(mode.lines(), f, rect),
            _ => (),
        }
    }

    fn binds_per_mode(&self, f: &mut Frame, rect: &Rect, mode: MenuMode) {
        if mode == MenuMode::Navigate(Navigate::Trash) {
            return;
        }
        let p_rect = rect.offseted(2, rect.height.saturating_sub(2));
        Span::styled(
            mode.binds_per_mode(),
            MENU_STYLES.get().expect("Menu colors should be set").second,
        )
        .render(p_rect, f.buffer_mut());
    }

    fn input_simple(lines: &[&str], f: &mut Frame, rect: &Rect) {
        let mut p_rect = rect.offseted(4, ContentWindow::WINDOW_MARGIN_TOP_U16);
        p_rect.height = p_rect.height.saturating_sub(2);
        Self::render_content(lines, f, &p_rect, 0, 0);
    }

    fn navigate(&self, navigate: Navigate, f: &mut Frame, rect: &Rect) {
        if navigate.simple_draw_menu() {
            return self.status.menu.draw_navigate(f, rect, navigate);
        }
        match navigate {
            Navigate::Cloud => self.cloud(f, rect),
            Navigate::Context => self.context(f, rect),
            Navigate::TempMarks(_) => self.temp_marks(f, rect),
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

        let p_rect = rect.offseted(2, rect.height.saturating_sub(2));
        Span::styled(
            &trash.help,
            MENU_STYLES.get().expect("Menu colors should be set").second,
        )
        .render(p_rect, f.buffer_mut());
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
        let p_rect = rect.offseted(2, 2);
        Span::styled(
            desc,
            MENU_STYLES
                .get()
                .expect("Menu colors should be set")
                .palette_4,
        )
        .render(p_rect, f.buffer_mut());
        cloud.draw_menu(f, rect, &self.status.menu.window)
    }

    fn picker(&self, f: &mut Frame, rect: &Rect) {
        let selectable = &self.status.menu.picker;
        selectable.draw_menu(f, rect, &self.status.menu.window);
        if let Some(desc) = &selectable.desc {
            let p_rect = rect.offseted(2, 1);
            Span::styled(
                desc,
                MENU_STYLES.get().expect("Menu colors should be set").second,
            )
            .render(p_rect, f.buffer_mut());
        }
    }

    fn temp_marks(&self, f: &mut Frame, rect: &Rect) {
        let selectable = &self.status.menu.temp_marks;
        selectable.draw_menu(f, rect, &self.status.menu.window);
    }

    fn context(&self, f: &mut Frame, rect: &Rect) {
        self.context_selectable(f, rect);
        self.context_more_infos(f, rect)
    }

    fn context_selectable(&self, f: &mut Frame, rect: &Rect) {
        self.status
            .menu
            .context
            .draw_menu(f, rect, &self.status.menu.window);
    }

    fn context_more_infos(&self, f: &mut Frame, rect: &Rect) {
        let space_used = self.status.menu.context.content.len() as u16;
        let more_info = MoreInfos::new(
            &self.tab.current_file().unwrap(),
            &self.status.internal_settings.opener,
        )
        .to_lines();
        Self::render_content(&more_info, f, rect, 4, 3 + space_used);
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
            let Ok(fileinfo) = FileInfo::new(selected, &self.tab.users) else {
                return;
            };
            let p_rect = rect.offseted(2, 2);
            Span::styled(
                fileinfo.format_metadata(6, 6).unwrap_or_default(),
                fileinfo.style(),
            )
            .render(p_rect, f.buffer_mut());
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
        let text_content: Vec<_> = self
            .status
            .menu
            .flagged
            .content()
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        Self::render_content(
            &text_content,
            f,
            rect,
            4,
            2 + ContentWindow::WINDOW_MARGIN_TOP_U16,
        );
    }

    fn confirm_bulk(&self, f: &mut Frame, rect: &Rect) {
        let content = self.status.menu.bulk.format_confirmation();
        Self::render_content(
            &content,
            f,
            rect,
            4,
            2 + ContentWindow::WINDOW_MARGIN_TOP_U16,
        );
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
        let content: Vec<_> = self
            .status
            .menu
            .trash
            .content()
            .iter()
            .map(|trashinfo| trashinfo.to_string())
            .collect();
        let mut p_rect = rect.offseted(4, 4);
        p_rect.height = p_rect.height.saturating_sub(1);
        Self::render_content(&content, f, &p_rect, 0, 0);
    }

    fn content_line(f: &mut Frame, rect: &Rect, row: u16, text: &str, style: Style) {
        let p_rect = rect.offseted(4, row + ContentWindow::WINDOW_MARGIN_TOP_U16);
        Span::styled(text, style).render(p_rect, f.buffer_mut());
    }
}

struct MenuFirstLine {
    content: Vec<String>,
}

impl Draw for MenuFirstLine {
    fn draw(&self, f: &mut Frame, rect: &Rect) {
        let spans: Vec<_> = std::iter::zip(
            self.content.iter(),
            MENU_STYLES
                .get()
                .expect("Menu colors should be set")
                .palette()
                .iter()
                .cycle(),
        )
        .map(|(text, style)| Span::styled(text, *style))
        .collect();
        let p_rect = rect.offseted(2, 0);
        Line::from(spans).render(p_rect, f.buffer_mut());
    }
}

impl MenuFirstLine {
    fn new(status: &Status) -> Self {
        Self {
            content: status.current_tab().menu_mode.line_display(status),
        }
    }
}

/// Methods used to create the various rects
struct Rects;

impl Rects {
    const FILES_WITH_LOGLINE: &[Constraint] = &[
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ];

    const FILES_WITHOUT_LOGLINE: &[Constraint] = &[
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ];

    /// Main rect of the application
    fn full_rect(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    /// Main rect but inside its border
    fn inside_border_rect(width: u16, height: u16) -> Rect {
        Rect::new(1, 1, width.saturating_sub(2), height.saturating_sub(2))
    }

    /// Horizontal split the inside rect in two
    fn left_right_inside_rects(rect: Rect) -> Rc<[Rect]> {
        Layout::new(
            Direction::Horizontal,
            [Constraint::Min(rect.width / 2), Constraint::Fill(1)],
        )
        .split(rect)
    }

    /// Bordered rects of the four windows
    fn dual_bordered_rect(
        parent_wins: Rc<[Rect]>,
        have_menu_left: bool,
        have_menu_right: bool,
    ) -> Vec<Rect> {
        let mut bordered_wins =
            Self::vertical_split_border(parent_wins[0], have_menu_left).to_vec();
        bordered_wins
            .append(&mut Self::vertical_split_border(parent_wins[1], have_menu_right).to_vec());
        bordered_wins
    }

    /// Inside rects of the four windows.
    fn dual_inside_rect(rect: Rect, have_menu_left: bool, have_menu_right: bool) -> Vec<Rect> {
        let left_right = Self::left_right_rects(rect);
        let mut areas = Self::vertical_split_inner(left_right[0], have_menu_left).to_vec();
        areas.append(&mut Self::vertical_split_inner(left_right[2], have_menu_right).to_vec());
        areas
    }

    /// Main inside rect for left and right.
    /// It also returns a padding rect which should be ignored by the caller.
    fn left_right_rects(rect: Rect) -> Rc<[Rect]> {
        Layout::new(
            Direction::Horizontal,
            [
                Constraint::Min(rect.width / 2 - 1),
                Constraint::Max(2),
                Constraint::Min(rect.width / 2 - 2),
            ],
        )
        .split(rect)
    }

    /// Vertical split used to split the inside windows of a pane, left or right.
    /// It also recturns a padding rect which should be ignored by the caller.
    fn vertical_split_inner(parent_win: Rect, have_menu: bool) -> Rc<[Rect]> {
        if have_menu {
            Layout::new(
                Direction::Vertical,
                [
                    Constraint::Min(parent_win.height / 2 - 1),
                    Constraint::Max(2),
                    Constraint::Fill(1),
                ],
            )
            .split(parent_win)
        } else {
            Rc::new([parent_win, Rect::default(), Rect::default()])
        }
    }

    /// Vertical split used to create the bordered rects of a pane, left or right.
    fn vertical_split_border(parent_win: Rect, have_menu: bool) -> Rc<[Rect]> {
        let percent = if have_menu { 50 } else { 100 };
        Layout::new(
            Direction::Vertical,
            [Constraint::Percentage(percent), Constraint::Fill(1)],
        )
        .split(parent_win)
    }

    /// Split the rect vertically like this:
    /// 1   :       header
    /// 1   :       copy progress bar or second line
    /// fill:       content
    /// 1   :       log line
    /// 1   :       footer
    fn files(rect: &Rect, use_log_line: bool) -> Rc<[Rect]> {
        Layout::new(
            Direction::Vertical,
            if use_log_line {
                Self::FILES_WITH_LOGLINE
            } else {
                Self::FILES_WITHOUT_LOGLINE
            },
        )
        .split(*rect)
    }

    fn fuzzy(area: &Rect) -> Rc<[Rect]> {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(*area)
    }
}

/// Is responsible for displaying content in the terminal.
/// It uses an already created terminal.
pub struct Display {
    /// The Crossterm terminal attached to the display.
    /// It will print every symbol shown on screen.
    term: Terminal<CrosstermBackend<Stdout>>,
    /// The ueberzug instance used to draw the images
    ueberzug: Arc<Mutex<Ueberzug>>,
}

impl Display {
    /// Returns a new `Display` instance from a terminal object.
    pub fn new(term: Terminal<CrosstermBackend<Stdout>>) -> Self {
        log_info!("starting display...");
        let ueberzug = Arc::new(Mutex::new(Ueberzug::default()));
        Self { term, ueberzug }
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
        io::stdout().flush().expect("Couldn't flush the stdout");
        if status.should_be_cleared() {
            self.term.clear().expect("Couldn't clear the terminal");
        }
        let Ok(Size { width, height }) = self.term.size() else {
            return;
        };
        let full_rect = Rects::full_rect(width, height);
        let inside_border_rect = Rects::inside_border_rect(width, height);
        let borders = Self::borders(status);
        if Self::use_dual_pane(status, width) {
            self.draw_dual(full_rect, inside_border_rect, borders, status);
        } else {
            self.draw_single(
                full_rect,
                inside_border_rect,
                borders,
                status,
                self.ueberzug.clone(),
            );
        };
    }

    /// Left File, Left Menu, Right File, Right Menu
    fn borders(status: &Status) -> [Style; 4] {
        let menu_styles = MENU_STYLES.get().expect("MENU_STYLES should be set");
        let mut borders = [menu_styles.inert_border; 4];
        let selected_border = menu_styles.selected_border;
        borders[status.focus.index()] = selected_border;
        borders
    }

    /// True iff we need to display both panes
    fn use_dual_pane(status: &Status, width: u16) -> bool {
        status.session.dual() && width > MIN_WIDTH_FOR_DUAL_PANE
    }

    fn draw_dual(
        &mut self,
        full_rect: Rect,
        inside_border_rect: Rect,
        borders: [Style; 4],
        status: &Status,
    ) {
        let (file_left, file_right) = FilesBuilder::dual(status, full_rect.width);
        let menu_left = Menu::new(status, 0);
        let menu_right = Menu::new(status, 1);
        let parent_wins = Rects::left_right_inside_rects(full_rect);
        let have_menu_left = status.tabs[0].need_menu_window();
        let have_menu_right = status.tabs[1].need_menu_window();
        let bordered_wins = Rects::dual_bordered_rect(parent_wins, have_menu_left, have_menu_right);
        let inside_wins =
            Rects::dual_inside_rect(inside_border_rect, have_menu_left, have_menu_right);
        self.render_dual(
            borders,
            bordered_wins,
            inside_wins,
            (file_left, file_right),
            (menu_left, menu_right),
            self.ueberzug.clone(),
        );
    }

    fn render_dual(
        &mut self,
        borders: [Style; 4],
        bordered_wins: Vec<Rect>,
        inside_wins: Vec<Rect>,
        files: (Files, Files),
        menus: (Menu, Menu),
        ueberzug: Arc<Mutex<Ueberzug>>,
    ) {
        self.term
            .draw(|f| {
                // 0 File Left | 3 File Right
                // 1 padding   | 4 padding
                // 2 Menu Left | 5 Menu Right
                Self::draw_dual_borders(borders, f, &bordered_wins);
                files.0.draw(f, &inside_wins[0], ueberzug.clone());
                menus.0.draw(f, &inside_wins[2]);
                files.1.draw(f, &inside_wins[3], ueberzug);
                menus.1.draw(f, &inside_wins[5]);
            })
            .unwrap();
    }

    fn draw_single(
        &mut self,
        rect: Rect,
        inside_border_rect: Rect,
        borders: [Style; 4],
        status: &Status,
        ueberzug: Arc<Mutex<Ueberzug>>,
    ) {
        let file_left = FilesBuilder::single(status);
        let menu_left = Menu::new(status, 0);
        let need_menu = status.tabs[0].need_menu_window();
        let bordered_wins = Rects::vertical_split_border(rect, need_menu);
        let inside_wins = Rects::vertical_split_inner(inside_border_rect, need_menu);
        self.render_single(
            borders,
            bordered_wins,
            inside_wins,
            file_left,
            menu_left,
            ueberzug,
        )
    }

    fn render_single(
        &mut self,
        borders: [Style; 4],
        bordered_wins: Rc<[Rect]>,
        inside_wins: Rc<[Rect]>,
        file_left: Files,
        menu_left: Menu,
        ueberzug: Arc<Mutex<Ueberzug>>,
    ) {
        self.term
            .draw(|f| {
                Self::draw_single_borders(borders, f, &bordered_wins);
                file_left.draw(f, &inside_wins[0], ueberzug);
                menu_left.draw(f, &inside_wins[2]);
            })
            .unwrap();
    }

    fn draw_n_borders(n: usize, borders: [Style; 4], f: &mut Frame, wins: &[Rect]) {
        for i in 0..n {
            let bordered_block = Block::default()
                .borders(Borders::ALL)
                .border_style(borders[i]);
            f.render_widget(bordered_block, wins[i]);
        }
    }

    fn draw_dual_borders(borders: [Style; 4], f: &mut Frame, wins: &[Rect]) {
        Self::draw_n_borders(4, borders, f, wins)
    }

    fn draw_single_borders(borders: [Style; 4], f: &mut Frame, wins: &[Rect]) {
        Self::draw_n_borders(2, borders, f, wins)
    }

    pub fn clear_ueberzug(&mut self) -> Result<()> {
        self.ueberzug
            .lock()
            .expect("Couldn't lock ueberzug")
            .clear_last()
    }

    pub fn restore_terminal(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.term.backend_mut(), LeaveAlternateScreen)?;
        self.term.show_cursor()?;
        Ok(())
    }
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
