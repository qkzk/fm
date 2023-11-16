use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::info;
use tuikit::attr::*;
use tuikit::event::Event;
use tuikit::prelude::*;
use tuikit::term::Term;

use crate::app::Status;
use crate::app::Tab;
use crate::completion::InputCompleted;
use crate::constant_strings_paths::{
    ENCRYPTED_DEVICE_BINDS, HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE, LOG_FIRST_SENTENCE,
    LOG_SECOND_SENTENCE, TRASH_CONFIRM_LINE,
};
use crate::content_window::ContentWindow;
use crate::display_mode::calculate_top_bottom;
use crate::display_mode::shorten_path;
use crate::display_mode::{fileinfo_attr, FileInfo};
use crate::display_mode::{Preview, TextKind, Window};
use crate::edit_mode::MountHelper;
use crate::edit_mode::SelectableContent;
use crate::edit_mode::Trash;
use crate::log::read_last_log_line;
use crate::mode::{DisplayMode, EditMode, InputSimple, MarkAction, Navigate, NeedConfirmation};
use crate::utils::path_to_string;

/// Iter over the content, returning a triplet of `(index, line, attr)`.
macro_rules! enumerated_colored_iter {
    ($t:ident) => {
        std::iter::zip($t.iter().enumerate(), MENU_COLORS.iter().cycle())
            .map(|((index, line), attr)| (index, line, attr))
    };
}

/// At least 120 chars width to display 2 tabs.
pub const MIN_WIDTH_FOR_DUAL_PANE: usize = 120;

const FIRST_LINE_COLORS: [Attr; 7] = [
    color_to_attr(Color::Rgb(231, 162, 156)),
    color_to_attr(Color::Rgb(144, 172, 186)),
    color_to_attr(Color::Rgb(214, 125, 83)),
    color_to_attr(Color::Rgb(91, 152, 119)),
    color_to_attr(Color::Rgb(152, 87, 137)),
    color_to_attr(Color::Rgb(230, 189, 87)),
    color_to_attr(Color::Rgb(251, 133, 0)),
];

const MENU_COLORS: [Attr; 10] = [
    color_to_attr(Color::Rgb(236, 250, 250)),
    color_to_attr(Color::Rgb(221, 242, 209)),
    color_to_attr(Color::Rgb(205, 235, 197)),
    color_to_attr(Color::Rgb(190, 227, 186)),
    color_to_attr(Color::Rgb(174, 220, 174)),
    color_to_attr(Color::Rgb(159, 212, 163)),
    color_to_attr(Color::Rgb(174, 220, 174)),
    color_to_attr(Color::Rgb(190, 227, 186)),
    color_to_attr(Color::Rgb(205, 235, 197)),
    color_to_attr(Color::Rgb(221, 242, 209)),
];

const ATTR_YELLOW_BOLD: Attr = Attr {
    fg: Color::YELLOW,
    bg: Color::Default,
    effect: Effect::BOLD,
};

/// Simple struct to read the events.
pub struct EventReader {
    term: Arc<Term>,
}

impl EventReader {
    /// Creates a new instance with an Arc to a terminal.
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    /// Returns the events as they're received. Wait indefinitely for a new one.
    /// We should spend most of the application life here, doing nothing :)
    pub fn poll_event(&self) -> Result<Event> {
        Ok(self.term.poll_event()?)
    }

    /// Height of the current terminal
    pub fn term_height(&self) -> Result<usize> {
        Ok(self.term.term_size()?.1)
    }
}

macro_rules! impl_preview {
    ($text:ident, $tab:ident, $length:ident, $canvas:ident, $line_number_width:ident, $window:ident) => {
        for (i, line) in (*$text).window($window.top, $window.bottom, $length) {
            let row = calc_line_row(i, $window);
            $canvas.print(row, $line_number_width + 3, line)?;
        }
    };
}

enum TabPosition {
    Left,
    Right,
}

/// Bunch of attributes describing the state of a main window
/// relatively to other windows
struct WinMainAttributes {
    /// horizontal position, in cells
    x_position: usize,
    /// is this the first (left) or second (right) window ?
    is_left: TabPosition,
    /// is this tab selected ?
    is_selected: bool,
    /// is there a secondary window ?
    has_window_below: bool,
}

impl WinMainAttributes {
    fn new(
        x_position: usize,
        is_second: TabPosition,
        is_selected: bool,
        has_window_below: bool,
    ) -> Self {
        Self {
            x_position,
            is_left: is_second,
            is_selected,
            has_window_below,
        }
    }

    fn is_right(&self) -> bool {
        matches!(self.is_left, TabPosition::Right)
    }
}

struct WinMain<'a> {
    status: &'a Status,
    tab: &'a Tab,
    disk_space: &'a str,
    attributes: WinMainAttributes,
}

impl<'a> Draw for WinMain<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        canvas.clear()?;
        if self.status.dual_pane && self.is_right() && self.status.preview_second {
            self.draw_preview_as_second_pane(canvas)?;
            return Ok(());
        }
        let opt_index = self.draw_content(canvas)?;
        WinMainFirstLine::new(
            self.disk_space,
            opt_index,
            self.attributes.is_selected,
            self.status,
        )?
        .draw(canvas)?;
        // self.first_line(self.disk_space, canvas, opt_index)?;
        Ok(())
    }
}

impl<'a> Widget for WinMain<'a> {}

impl<'a> WinMain<'a> {
    const ATTR_LINE_NR: Attr = color_to_attr(Color::CYAN);

    fn new(
        status: &'a Status,
        index: usize,
        disk_space: &'a str,
        attributes: WinMainAttributes,
    ) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
            disk_space,
            attributes,
        }
    }

    fn is_right(&self) -> bool {
        self.attributes.is_right()
    }

    fn draw_content(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        match &self.tab.display_mode {
            DisplayMode::Normal => self.draw_files(canvas),
            DisplayMode::Tree => self.draw_tree(canvas),
            DisplayMode::Preview => self.draw_preview(self.tab, &self.tab.window, canvas),
        }
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Only the files around the selected one are displayed.
    /// We reverse the attributes of the selected one, underline the flagged files.
    /// When we display a simpler version, the second line is used to display the
    /// metadata of the selected file.
    fn draw_files(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        let len = self.tab.path_content.content.len();
        let group_size: usize;
        let owner_size: usize;
        if self.status.display_full {
            group_size = self.tab.path_content.group_column_width();
            owner_size = self.tab.path_content.owner_column_width();
        } else {
            group_size = 0;
            owner_size = 0;
        }

        for (i, file) in self
            .tab
            .path_content
            .content
            .iter()
            .enumerate()
            .take(min(len, self.tab.window.bottom))
            .skip(self.tab.window.top)
        {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - self.tab.window.top;
            let mut attr = fileinfo_attr(file);
            let string = if self.status.display_full {
                file.format(owner_size, group_size)?
            } else {
                file.format_simple()?
            };
            if self.status.flagged.contains(&file.path) {
                attr.effect |= Effect::BOLD;
                canvas.print_with_attr(row, 0, "█", ATTR_YELLOW_BOLD)?;
            }
            canvas.print_with_attr(row, 1, &string, attr)?;
        }
        let _ = WinMainSecondLine::new(self.status, self.tab).draw(canvas);
        if !self.attributes.has_window_below {
            let _ = LogLine {}.draw(canvas);
        }
        Ok(None)
    }

    fn draw_tree(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        let left_margin = if self.status.display_full { 1 } else { 3 };
        let (_, height) = canvas.size()?;
        let (selected_index, content) = self.tab.tree.into_navigable_content(&self.tab.users);
        let (top, bottom) = calculate_top_bottom(selected_index, height);
        let length = content.len();

        for (i, (metadata, prefix, colored_string)) in content
            .iter()
            .enumerate()
            .skip(top)
            .take(min(length, bottom + 1))
        {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - top;
            let mut attr = colored_string.color_effect.attr();
            if self.status.flagged.contains(&colored_string.path) {
                attr.effect |= Effect::BOLD;
                canvas.print_with_attr(row, 0, "█", ATTR_YELLOW_BOLD)?;
            }

            let col_metadata = if self.status.display_full {
                canvas.print_with_attr(row, left_margin, metadata, attr)?
            } else {
                0
            };
            let offset = if i == 0 { 1 } else { 0 };

            let col_tree_prefix = canvas.print(row, left_margin + col_metadata + offset, prefix)?;
            canvas.print_with_attr(
                row,
                left_margin + col_metadata + col_tree_prefix + offset,
                &colored_string.text,
                attr,
            )?;
        }
        let _ = WinMainSecondLine::new(self.status, self.tab).draw(canvas);
        Ok(Some(selected_index))
    }

    fn draw_line_number(
        row_position_in_canvas: usize,
        line_number_to_print: usize,
        canvas: &mut dyn Canvas,
    ) -> Result<usize> {
        Ok(canvas.print_with_attr(
            row_position_in_canvas,
            0,
            &line_number_to_print.to_string(),
            Self::ATTR_LINE_NR,
        )?)
    }

    /// Display a scrollable preview of a file.
    /// Multiple modes are supported :
    /// if the filename extension is recognized, the preview is highlighted,
    /// if the file content is recognized as binary, an hex dump is previewed with 16 bytes lines,
    /// else the content is supposed to be text and shown as such.
    /// It may fail to recognize some usual extensions, notably `.toml`.
    /// It may fail to recognize small files (< 1024 bytes).
    fn draw_preview(
        &self,
        tab: &Tab,
        window: &ContentWindow,
        canvas: &mut dyn Canvas,
    ) -> Result<Option<usize>> {
        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                for (i, vec_line) in (*syntaxed).window(window.top, window.bottom, length) {
                    let row_position = calc_line_row(i, window);
                    Self::draw_line_number(row_position, i + 1, canvas)?;
                    for token in vec_line.iter() {
                        token.print(canvas, row_position, line_number_width)?;
                    }
                }
            }
            Preview::Binary(bin) => {
                let line_number_width_hex = format!("{:x}", bin.len() * 16).len();

                for (i, line) in (*bin).window(window.top, window.bottom, length) {
                    let row = calc_line_row(i, window);

                    canvas.print_with_attr(
                        row,
                        0,
                        &format_line_nr_hex(i + 1 + window.top, line_number_width_hex),
                        Self::ATTR_LINE_NR,
                    )?;
                    line.print_bytes(canvas, row, line_number_width_hex + 1);
                    line.print_ascii(canvas, row, line_number_width_hex + 43);
                }
            }
            Preview::Ueberzug(image) => {
                let (width, height) = canvas.size()?;
                image.match_index()?;
                image.ueberzug(
                    self.attributes.x_position as u16 + 2,
                    3,
                    width as u16 - 2,
                    height as u16 - 2,
                );
            }
            Preview::Directory(directory) => {
                for (i, (_, prefix, colored_string)) in
                    (directory).window(window.top, window.bottom, length)
                {
                    let row = calc_line_row(i, window);
                    let col = canvas.print(row, line_number_width, prefix)?;

                    canvas.print_with_attr(
                        row,
                        line_number_width + col + 1,
                        &colored_string.text,
                        colored_string.color_effect.attr(),
                    )?;
                }
            }
            Preview::ColoredText(colored_text) => {
                for (i, line) in colored_text.window(window.top, window.bottom, length) {
                    let row = calc_line_row(i, window);
                    let mut col = 3;
                    for (chr, attr) in skim::AnsiString::parse(line).iter() {
                        col += canvas.print_with_attr(row, col, &chr.to_string(), attr)?;
                    }
                }
            }
            Preview::Archive(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }
            Preview::Media(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }
            Preview::Text(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }
            Preview::Diff(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }
            Preview::Iso(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }
            Preview::Socket(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }
            Preview::BlockDevice(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }
            Preview::FifoCharDevice(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window)
            }

            Preview::Empty => (),
        }
        Ok(None)
    }

    fn draw_preview_as_second_pane(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let tab = &self.status.tabs[1];
        self.draw_preview(tab, &tab.window, canvas)?;
        draw_colored_strings(
            0,
            0,
            &WinMainFirstLine::default_preview_first_line(self.status, tab),
            canvas,
            false,
        )?;
        Ok(())
    }
}

struct WinMainFirstLine {
    content: Vec<String>,
    is_selected: bool,
}

impl Draw for WinMainFirstLine {
    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    /// Returns the result of the number of printed chars.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        draw_colored_strings(0, 0, &self.content, canvas, self.is_selected)?;
        Ok(())
    }
}

impl WinMainFirstLine {
    fn new(
        disk_space: &str,
        opt_index: Option<usize>,
        is_selected: bool,
        status: &Status,
    ) -> Result<Self> {
        let tab = status.selected_non_mut();
        let content = match tab.display_mode {
            DisplayMode::Normal | DisplayMode::Tree => {
                Self::normal_first_row(status, tab, disk_space, opt_index)?
            }
            DisplayMode::Preview => match &tab.preview {
                Preview::Text(text_content) => match text_content.kind {
                    TextKind::HELP => Self::help_first_row(),
                    TextKind::LOG => Self::log_first_row(),
                    _ => Self::default_preview_first_line(status, tab),
                },
                _ => Self::default_preview_first_line(status, tab),
            },
        };
        Ok(Self {
            content,
            is_selected,
        })
    }

    fn normal_first_row(
        status: &Status,
        tab: &Tab,
        disk_space: &str,
        opt_index: Option<usize>,
    ) -> Result<Vec<String>> {
        Ok(vec![
            Self::shorten_path(tab)?,
            Self::first_row_selected_file(tab)?,
            Self::first_row_position(tab, opt_index),
            Self::used_space(tab),
            Self::disk_space(disk_space),
            Self::git_string(tab)?,
            Self::first_row_flags(status),
            Self::sort_kind(tab),
        ])
    }

    fn shorten_path(tab: &Tab) -> Result<String> {
        Ok(format!(" {}", shorten_path(&tab.path_content.path, None)?))
    }

    fn first_row_selected_file(tab: &Tab) -> Result<String> {
        match tab.display_mode {
            DisplayMode::Tree => Ok(format!(
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

    fn first_row_position(tab: &Tab, opt_index: Option<usize>) -> String {
        if matches!(tab.display_mode, DisplayMode::Tree) {
            let Some(selected_index) = opt_index else {
                return "".to_owned();
            };
            return format!(
                " {position} / {len} ",
                position = selected_index + 1,
                len = tab.tree.len()
            );
        };
        format!(
            " {index} / {len} ",
            index = tab.path_content.index + 1,
            len = tab.path_content.len()
        )
    }

    fn used_space(tab: &Tab) -> String {
        format!("{}  ", tab.path_content.used_space())
    }

    fn disk_space(disk_space: &str) -> String {
        format!(" Avail: {disk_space}  ")
    }

    fn git_string(tab: &Tab) -> Result<String> {
        Ok(format!(" {} ", tab.path_content.git_string()?))
    }

    fn sort_kind(tab: &Tab) -> String {
        format!(" {} ", &tab.path_content.sort_kind)
    }

    fn first_row_flags(status: &Status) -> String {
        let nb_flagged = status.flagged.len();
        let flag_string = if status.flagged.len() > 1 {
            "flags"
        } else {
            "flag"
        };
        format!(" {nb_flagged} {flag_string} ",)
    }

    fn help_first_row() -> Vec<String> {
        vec![
            HELP_FIRST_SENTENCE.to_owned(),
            format!(" Version: {v} ", v = std::env!("CARGO_PKG_VERSION")),
            HELP_SECOND_SENTENCE.to_owned(),
        ]
    }

    fn log_first_row() -> Vec<String> {
        vec![
            LOG_FIRST_SENTENCE.to_owned(),
            LOG_SECOND_SENTENCE.to_owned(),
        ]
    }

    fn pick_previewed_fileinfo(status: &Status) -> Result<FileInfo> {
        if status.dual_pane && status.preview_second {
            status.tabs[0].selected()
        } else {
            status.selected_non_mut().selected()
        }
    }

    fn default_preview_first_line(status: &Status, tab: &Tab) -> Vec<String> {
        if let Ok(fileinfo) = Self::pick_previewed_fileinfo(status) {
            let mut strings = vec![" Preview ".to_owned()];
            if !tab.preview.is_empty() {
                let index = match &tab.preview {
                    Preview::Ueberzug(image) => image.index + 1,
                    _ => tab.window.bottom,
                };
                strings.push(format!(" {index} / {len} ", len = tab.preview.len()));
            };
            strings.push(format!(" {} ", fileinfo.path.display()));
            strings
        } else {
            vec!["".to_owned()]
        }
    }
}

struct WinMainSecondLine {
    content: Option<String>,
    attr: Option<Attr>,
}

impl Draw for WinMainSecondLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        match (&self.content, &self.attr) {
            (Some(content), Some(attr)) => canvas.print_with_attr(1, 1, content, *attr)?,
            _ => 0,
        };
        Ok(())
    }
}

impl WinMainSecondLine {
    fn new(status: &Status, tab: &Tab) -> Self {
        let (content, attr) = match tab.display_mode {
            DisplayMode::Normal | DisplayMode::Tree => {
                if !status.display_full {
                    if let Ok(file) = tab.selected() {
                        Self::second_line_detailed(&file)
                    } else {
                        (None, None)
                    }
                } else {
                    Self::second_line_simple(status)
                }
            }
            _ => (None, None),
        };
        Self { content, attr }
    }

    fn second_line_detailed(file: &FileInfo) -> (Option<String>, Option<Attr>) {
        let owner_size = file.owner.len();
        let group_size = file.group.len();
        let mut attr = fileinfo_attr(file);
        attr.effect ^= Effect::REVERSE;

        (
            Some(file.format(owner_size, group_size).unwrap_or_default()),
            Some(attr),
        )
    }

    fn second_line_simple(status: &Status) -> (Option<String>, Option<Attr>) {
        (
            Some(status.selected_non_mut().filter.to_string()),
            Some(ATTR_YELLOW_BOLD),
        )
    }
}

struct LogLine {}

impl Draw for LogLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let (_, height) = canvas.size()?;
        canvas.print_with_attr(height - 1, 4, &read_last_log_line(), ATTR_YELLOW_BOLD)?;
        Ok(())
    }
}

struct WinSecondary<'a> {
    status: &'a Status,
    tab: &'a Tab,
}
impl<'a> Draw for WinSecondary<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        canvas.clear()?;
        match self.tab.edit_mode {
            EditMode::Navigate(mode) => self.draw_navigate(mode, canvas),
            EditMode::NeedConfirmation(mode) => self.draw_confirm(mode, canvas),
            EditMode::InputCompleted(_) => self.draw_completion(canvas),
            EditMode::InputSimple(mode) => Self::draw_static_lines(mode.lines(), canvas),
            _ => Ok(()),
        }?;
        self.draw_cursor(canvas)?;
        WinSecondaryFirstLine::new(self.tab)?.draw(canvas)
    }
}

impl<'a> WinSecondary<'a> {
    const ATTR_YELLOW: Attr = color_to_attr(Color::YELLOW);
    const EDIT_BOX_OFFSET: usize = 11;
    const SORT_CURSOR_OFFSET: usize = 39;
    const PASSWORD_CURSOR_OFFSET: usize = 9;

    fn new(status: &'a Status, index: usize) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
        }
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn draw_completion(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = &self.tab.completion.proposals;
        for (row, candidate, attr) in enumerated_colored_iter!(content) {
            let attr = self.tab.completion.attr(row, attr);
            Self::draw_content_line(canvas, row, candidate, attr)?;
        }
        Ok(())
    }

    fn draw_static_lines(lines: &[&str], canvas: &mut dyn Canvas) -> Result<()> {
        for (row, line, attr) in enumerated_colored_iter!(lines) {
            Self::draw_content_line(canvas, row, line, *attr)?
        }
        Ok(())
    }

    /// Display a cursor in the top row, at a correct column.
    fn draw_cursor(&self, canvas: &mut dyn Canvas) -> Result<()> {
        match self.tab.edit_mode {
            EditMode::Navigate(_) | EditMode::Nothing => {
                canvas.show_cursor(false)?;
            }
            EditMode::InputSimple(InputSimple::Sort) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, Self::SORT_CURSOR_OFFSET)?;
            }
            EditMode::InputSimple(InputSimple::Password(_, _, _)) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(
                    0,
                    Self::PASSWORD_CURSOR_OFFSET + self.tab.input.cursor_index,
                )?;
            }
            EditMode::InputSimple(_) | EditMode::InputCompleted(_) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, Self::EDIT_BOX_OFFSET + self.tab.input.cursor_index)?;
            }
            EditMode::NeedConfirmation(confirmed_action) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, confirmed_action.cursor_offset())?;
            }
        }
        Ok(())
    }

    fn draw_navigate(&self, navigable_mode: Navigate, canvas: &mut dyn Canvas) -> Result<()> {
        match navigable_mode {
            Navigate::Bulk => self.draw_bulk(canvas),
            Navigate::CliInfo => self.draw_cli_info(canvas),
            Navigate::Compress => self.draw_compress(canvas),
            Navigate::EncryptedDrive => self.draw_encrypted_drive(canvas),
            Navigate::History => self.draw_history(canvas),
            Navigate::Jump => self.draw_destination(canvas, &self.status.flagged),
            Navigate::Marks(_) => self.draw_marks(canvas),
            Navigate::RemovableDevices => self.draw_removable(canvas),
            Navigate::ShellMenu => self.draw_shell_menu(canvas),
            Navigate::Shortcut => self.draw_destination(canvas, &self.tab.shortcut),
            Navigate::Trash => self.draw_trash(canvas),
        }
    }

    /// Display the possible destinations from a selectable content of PathBuf.
    fn draw_destination(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<PathBuf>,
    ) -> Result<()> {
        canvas.print(0, 0, "Go to...")?;
        let content = selectable.content();
        for (row, path, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, attr);
            Self::draw_content_line(
                canvas,
                row,
                path.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_history(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.tab.history;
        canvas.print(0, 0, "Go to...")?;
        let content = selectable.content();
        for (row, pair, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, attr);
            Self::draw_content_line(
                canvas,
                row,
                pair.0.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_bulk(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if let Some(selectable) = &self.status.bulk {
            canvas.print(0, 0, "Action...")?;
            let content = selectable.content();
            for (row, text, attr) in enumerated_colored_iter!(content) {
                let attr = selectable.attr(row, attr);
                Self::draw_content_line(canvas, row, text, attr)?;
            }
        }
        Ok(())
    }

    fn draw_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let trash = &self.status.trash;
        if trash.content().is_empty() {
            self.draw_trash_is_empty(canvas)
        } else {
            self.draw_trash_content(canvas, trash)
        };
        Ok(())
    }

    fn draw_trash_content(&self, canvas: &mut dyn Canvas, trash: &Trash) {
        let _ = canvas.print(1, 2, TRASH_CONFIRM_LINE);
        let content = trash.content();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
            let attr = trash.attr(row, attr);
            let _ = Self::draw_content_line(canvas, row + 2, &trashinfo.to_string(), attr);
        }
    }

    fn draw_compress(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.status.compression;
        canvas.print_with_attr(
            2,
            2,
            "Archive and compress the flagged files.",
            Self::ATTR_YELLOW,
        )?;
        canvas.print_with_attr(3, 2, "Pick a compression algorithm.", Self::ATTR_YELLOW)?;
        let content = selectable.content();
        for (row, compression_method, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, attr);
            Self::draw_content_line(canvas, row + 3, &compression_method.to_string(), attr)?;
        }
        Ok(())
    }

    fn draw_marks(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 4, "mark  path", Self::ATTR_YELLOW)?;

        let content = self.status.marks.as_strings();
        for (row, line, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.marks.attr(row, attr);
            Self::draw_content_line(canvas, row, line, attr)?;
        }
        Ok(())
    }

    fn draw_shell_menu(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 1, "pick a command", Self::ATTR_YELLOW)?;

        let content = &self.status.shell_menu.content;
        for (row, (command, _), attr) in enumerated_colored_iter!(content) {
            let attr = self.status.shell_menu.attr(row, attr);
            Self::draw_content_line(canvas, row + 2, command, attr)?;
        }
        Ok(())
    }

    fn draw_cli_info(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 1, "pick a command", Self::ATTR_YELLOW)?;

        let content = &self.status.cli_info.content;
        for (row, command, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.cli_info.attr(row, attr);
            Self::draw_content_line(canvas, row + 2, command, attr)?;
        }
        Ok(())
    }

    fn draw_encrypted_drive(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.draw_mountable_devices(&self.status.encrypted_devices, canvas)
    }

    fn draw_removable(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if let Some(removables) = &self.status.removable_devices {
            self.draw_mountable_devices(removables, canvas)?;
        }
        Ok(())
    }

    fn draw_mountable_devices<T>(
        &self,
        selectable: &impl SelectableContent<T>,
        canvas: &mut dyn Canvas,
    ) -> Result<()>
    where
        T: MountHelper,
    {
        canvas.print_with_attr(2, 3, ENCRYPTED_DEVICE_BINDS, Self::ATTR_YELLOW)?;
        for (i, device) in selectable.content().iter().enumerate() {
            self.draw_mountable_device(selectable, i, device, canvas)?
        }
        Ok(())
    }

    fn draw_mountable_device<T>(
        &self,
        selectable: &impl SelectableContent<T>,
        index: usize,
        device: &T,
        canvas: &mut dyn Canvas,
    ) -> Result<()>
    where
        T: MountHelper,
    {
        let row = calc_line_row(index, &self.tab.window) + 2;
        let attr = selectable.attr(index, &device.attr());
        canvas.print_with_attr(row, 3, &device.device_name()?, attr)?;
        Ok(())
    }

    /// Display a list of edited (deleted, copied, moved, trashed) files for confirmation
    fn draw_confirm(
        &self,
        confirmed_mode: NeedConfirmation,
        canvas: &mut dyn Canvas,
    ) -> Result<()> {
        info!("confirmed action: {:?}", confirmed_mode);
        let dest = path_to_string(&self.tab.directory_of_selected()?);

        Self::draw_content_line(
            canvas,
            0,
            &confirmed_mode.confirmation_string(&dest),
            ATTR_YELLOW_BOLD,
        )?;
        match confirmed_mode {
            NeedConfirmation::EmptyTrash => self.draw_confirm_empty_trash(canvas)?,
            _ => self.draw_confirm_default(canvas)?,
        }
        Ok(())
    }

    fn draw_confirm_default(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = &self.status.flagged.content;
        for (row, path, attr) in enumerated_colored_iter!(content) {
            Self::draw_content_line(
                canvas,
                row + 2,
                path.to_str().context("Unreadable filename")?,
                *attr,
            )?;
        }
        Ok(())
    }

    fn draw_confirm_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        log::info!("draw_confirm_empty_trash");
        if self.status.trash.is_empty() {
            self.draw_trash_is_empty(canvas)
        } else {
            self.draw_confirm_non_empty_trash(canvas)?
        }
        Ok(())
    }

    fn draw_trash_is_empty(&self, canvas: &mut dyn Canvas) {
        let _ = Self::draw_content_line(canvas, 0, "Trash is empty", ATTR_YELLOW_BOLD);
    }

    fn draw_confirm_non_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = self.status.trash.content();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.trash.attr(row, attr);
            Self::draw_content_line(canvas, row + 4, &trashinfo.to_string(), attr)?
        }
        Ok(())
    }

    fn draw_content_line(
        canvas: &mut dyn Canvas,
        row: usize,
        text: &str,
        attr: tuikit::attr::Attr,
    ) -> Result<()> {
        canvas.print_with_attr(row + ContentWindow::WINDOW_MARGIN_TOP, 4, text, attr)?;
        Ok(())
    }
}

impl<'a> Widget for WinSecondary<'a> {}

struct WinSecondaryFirstLine {
    content: Vec<String>,
}

impl Draw for WinSecondaryFirstLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        draw_colored_strings(0, 1, &self.content, canvas, false)?;
        Ok(())
    }
}

impl WinSecondaryFirstLine {
    fn new(tab: &Tab) -> Result<Self> {
        let content = match tab.edit_mode {
            EditMode::NeedConfirmation(confirmed_action) => {
                vec![format!("{confirmed_action}"), " (y/n)".to_owned()]
            }
            EditMode::Navigate(Navigate::Marks(MarkAction::Jump)) => {
                vec!["Jump to...".to_owned()]
            }
            EditMode::Navigate(Navigate::Marks(MarkAction::New)) => {
                vec!["Save mark...".to_owned()]
            }
            EditMode::InputSimple(InputSimple::Password(password_kind, _encrypted_action, _)) => {
                info!("term: password");
                vec![format!("{password_kind}"), tab.input.password()]
            }
            EditMode::InputCompleted(mode) => {
                let mut completion_strings = vec![tab.edit_mode.to_string(), tab.input.string()];
                if let Some(completion) = tab.completion.complete_input_string(&tab.input.string())
                {
                    completion_strings.push(completion.to_owned())
                }
                if let InputCompleted::Exec = mode {
                    let selected_path = &tab.selected().context("can't parse path")?.path;
                    let selected_path = format!(" {}", selected_path.display());

                    completion_strings.push(selected_path);
                }
                completion_strings
            }
            _ => {
                vec![tab.edit_mode.to_string(), tab.input.string()]
            }
        };
        Ok(Self { content })
    }
}

/// Is responsible for displaying content in the terminal.
/// It uses an already created terminal.
pub struct Display {
    /// The Tuikit terminal attached to the display.
    /// It will print every symbol shown on screen.
    term: Arc<Term>,
}

impl Display {
    const SELECTED_BORDER: Attr = color_to_attr(Color::LIGHT_BLUE);
    const INERT_BORDER: Attr = color_to_attr(Color::Default);

    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    /// Used to force a display of the cursor before leaving the application.
    /// Most of the times we don't need a cursor and it's hidden. We have to
    /// do it unless the shell won't display a cursor anymore.
    pub fn show_cursor(&self) -> Result<()> {
        Ok(self.term.show_cursor(true)?)
    }

    fn hide_cursor(&self) -> Result<()> {
        self.term.set_cursor(0, 0)?;
        Ok(self.term.show_cursor(false)?)
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
    pub fn display_all(&mut self, status: &Status) -> Result<()> {
        self.hide_cursor()?;
        self.term.clear()?;

        let (width, _) = self.term.term_size()?;
        let disk_spaces = status.disk_spaces_per_tab();
        if status.dual_pane && width > MIN_WIDTH_FOR_DUAL_PANE {
            self.draw_dual_pane(status, &disk_spaces.0, &disk_spaces.1)?
        } else {
            self.draw_single_pane(status, &disk_spaces.0)?
        }

        Ok(self.term.present()?)
    }

    /// Hide the curose, clear the terminal and present.
    pub fn force_clear(&mut self) -> Result<()> {
        self.hide_cursor()?;
        self.term.clear()?;
        self.term.present()?;
        Ok(())
    }

    fn size_for_second_window(&self, tab: &Tab) -> Result<usize> {
        if tab.need_second_window() {
            Ok(self.height()? / 2)
        } else {
            Ok(0)
        }
    }

    fn vertical_split<'a>(
        &self,
        win_main: &'a WinMain,
        win_secondary: &'a WinSecondary,
        border: Attr,
        size: usize,
    ) -> Result<VSplit<'a>> {
        Ok(VSplit::default()
            .split(
                Win::new(win_main)
                    .basis(self.height()? - size)
                    .shrink(4)
                    .border(true)
                    .border_attr(border),
            )
            .split(
                Win::new(win_secondary)
                    .basis(size)
                    .shrink(0)
                    .border(true)
                    .border_attr(border),
            ))
    }

    fn borders(&self, status: &Status) -> (Attr, Attr) {
        if status.index == 0 {
            (Self::SELECTED_BORDER, Self::INERT_BORDER)
        } else {
            (Self::INERT_BORDER, Self::SELECTED_BORDER)
        }
    }

    fn draw_dual_pane(
        &mut self,
        status: &Status,
        disk_space_tab_0: &str,
        disk_space_tab_1: &str,
    ) -> Result<()> {
        let (width, _) = self.term.term_size()?;
        let (first_selected, second_selected) = (status.index == 0, status.index == 1);
        let attributes_left = WinMainAttributes::new(
            0,
            TabPosition::Left,
            first_selected,
            status.tabs[0].need_second_window(),
        );
        let win_main_left = WinMain::new(status, 0, disk_space_tab_0, attributes_left);
        let attributes_right = WinMainAttributes::new(
            width / 2,
            TabPosition::Right,
            second_selected,
            status.tabs[1].need_second_window(),
        );
        let win_main_right = WinMain::new(status, 1, disk_space_tab_1, attributes_right);
        let win_second_left = WinSecondary::new(status, 0);
        let win_second_right = WinSecondary::new(status, 1);
        let (border_left, border_right) = self.borders(status);
        let percent_left = self.size_for_second_window(&status.tabs[0])?;
        let percent_right = self.size_for_second_window(&status.tabs[1])?;
        let hsplit = HSplit::default()
            .split(self.vertical_split(
                &win_main_left,
                &win_second_left,
                border_left,
                percent_left,
            )?)
            .split(self.vertical_split(
                &win_main_right,
                &win_second_right,
                border_right,
                percent_right,
            )?);
        Ok(self.term.draw(&hsplit)?)
    }

    fn draw_single_pane(&mut self, status: &Status, disk_space_tab_0: &str) -> Result<()> {
        let attributes_left = WinMainAttributes::new(
            0,
            TabPosition::Left,
            true,
            status.tabs[0].need_second_window(),
        );
        let win_main_left = WinMain::new(status, 0, disk_space_tab_0, attributes_left);
        let win_second_left = WinSecondary::new(status, 0);
        let percent_left = self.size_for_second_window(&status.tabs[0])?;
        let win = self.vertical_split(
            &win_main_left,
            &win_second_left,
            Self::SELECTED_BORDER,
            percent_left,
        )?;
        Ok(self.term.draw(&win)?)
    }

    /// Reads and returns the `tuikit::term::Term` height.
    pub fn height(&self) -> Result<usize> {
        let (_, height) = self.term.term_size()?;
        Ok(height)
    }
}

fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
    format!("{line_nr:0width$x}")
}

const fn color_to_attr(color: Color) -> Attr {
    Attr {
        fg: color,
        bg: Color::Default,
        effect: Effect::empty(),
    }
}

fn draw_colored_strings(
    row: usize,
    offset: usize,
    strings: &[String],
    canvas: &mut dyn Canvas,
    effect_reverse: bool,
) -> Result<()> {
    let mut col = 1;
    for (text, attr) in std::iter::zip(strings.iter(), FIRST_LINE_COLORS.iter().cycle()) {
        let mut attr = *attr;
        if effect_reverse {
            attr.effect |= Effect::REVERSE;
        }
        col += canvas.print_with_attr(row, offset + col, text, attr)?;
    }
    Ok(())
}

fn calc_line_row(i: usize, window: &ContentWindow) -> usize {
    i + ContentWindow::WINDOW_MARGIN_TOP - window.top
}
