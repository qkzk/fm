use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::info;
use tuikit::attr::*;
use tuikit::event::Event;
use tuikit::prelude::*;
use tuikit::term::Term;

use crate::completion::InputCompleted;
use crate::compress::CompressionMethod;
use crate::config::Colors;
use crate::constant_strings_paths::{
    HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE, LOG_FIRST_SENTENCE, LOG_SECOND_SENTENCE,
};
use crate::content_window::ContentWindow;
use crate::fileinfo::{fileinfo_attr, shorten_path, FileInfo};
use crate::log::read_last_log_line;
use crate::mode::{InputSimple, MarkAction, Mode, Navigate, NeedConfirmation};
use crate::mount_help::MountHelper;
use crate::preview::{Preview, TextKind, Window};
use crate::selectable_content::SelectableContent;
use crate::status::Status;
use crate::tab::Tab;
use crate::trash::TrashInfo;

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

/// Bunch of attributes describing the state of a main window
/// relatively to other windows
struct WinMainAttributes {
    /// horizontal position, in cells
    x_position: usize,
    /// is this the first (false) or second (true) window ?
    is_second: bool,
    /// is this tab selected ?
    is_selected: bool,
    /// is there a secondary window ?
    has_window_below: bool,
}

impl WinMainAttributes {
    fn new(x_position: usize, is_second: bool, is_selected: bool, has_window_below: bool) -> Self {
        Self {
            x_position,
            is_second,
            is_selected,
            has_window_below,
        }
    }
}

struct WinMain<'a> {
    status: &'a Status,
    tab: &'a Tab,
    disk_space: &'a str,
    colors: &'a Colors,
    attributes: WinMainAttributes,
}

impl<'a> Draw for WinMain<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        canvas.clear()?;
        if self.status.dual_pane && self.attributes.is_second && self.status.preview_second {
            self.preview_as_second_pane(canvas)?;
            return Ok(());
        }
        match self.tab.mode {
            Mode::Preview => self.preview(self.tab, &self.tab.window, canvas),
            Mode::Tree => self.tree(self.status, self.tab, canvas),
            Mode::Normal => self.files(self.status, self.tab, canvas),
            _ => match self.tab.previous_mode {
                Mode::Tree => self.tree(self.status, self.tab, canvas),
                _ => self.files(self.status, self.tab, canvas),
            },
        }?;
        self.first_line(self.tab, self.disk_space, canvas)?;
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
        colors: &'a Colors,
        attributes: WinMainAttributes,
    ) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
            disk_space,
            colors,
            attributes,
        }
    }

    fn preview_as_second_pane(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let tab = &self.status.tabs[0];
        let (_, height) = canvas.size()?;
        self.preview(tab, &tab.preview.window_for_second_pane(height), canvas)?;
        draw_colored_strings(0, 0, self.default_preview_first_line(tab), canvas, false)?;
        Ok(())
    }

    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    /// Returns the result of the number of printed chars.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn first_line(&self, tab: &Tab, disk_space: &str, canvas: &mut dyn Canvas) -> Result<()> {
        draw_colored_strings(
            0,
            0,
            self.create_first_row(tab, disk_space)?,
            canvas,
            self.attributes.is_selected,
        )
    }

    fn second_line(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> Result<usize> {
        match tab.mode {
            Mode::Normal | Mode::Tree => {
                if !status.display_full {
                    let Some(file) = tab.selected() else {
                        return Ok(0);
                    };
                    self.second_line_detailed(file, canvas)
                } else {
                    self.second_line_simple(status, canvas)
                }
            }
            _ => Ok(0),
        }
    }

    fn second_line_detailed(&self, file: &FileInfo, canvas: &mut dyn Canvas) -> Result<usize> {
        let owner_size = file.owner.len();
        let group_size = file.group.len();
        let mut attr = fileinfo_attr(file, self.colors);
        attr.effect ^= Effect::REVERSE;
        Ok(canvas.print_with_attr(1, 0, &file.format(owner_size, group_size)?, attr)?)
    }

    fn second_line_simple(&self, status: &Status, canvas: &mut dyn Canvas) -> Result<usize> {
        Ok(canvas.print_with_attr(
            1,
            0,
            &format!("{}", &status.selected_non_mut().filter),
            ATTR_YELLOW_BOLD,
        )?)
    }

    fn normal_first_row(&self, disk_space: &str) -> Result<Vec<String>> {
        Ok(vec![
            format!(" {}", shorten_path(&self.tab.path_content.path, None)?),
            self.first_row_filename(),
            self.first_row_position(),
            format!("{}  ", self.tab.path_content.used_space()),
            format!(" Avail: {disk_space}  "),
            format!(" {} ", self.tab.path_content.git_string()?),
            self.first_row_flags(),
            format!(" {} ", &self.tab.path_content.sort_kind),
        ])
    }

    fn first_row_filename(&self) -> String {
        match self.tab.mode {
            Mode::Tree => "".to_owned(),
            _ => {
                if let Some(fileinfo) = self.tab.path_content.selected() {
                    let name = &fileinfo.filename;
                    if name == "." || name == ".." {
                        "".to_owned()
                    } else {
                        format!("/{name} ")
                    }
                } else {
                    "".to_owned()
                }
            }
        }
    }

    fn first_row_position(&self) -> String {
        format!(
            " {} / {} ",
            self.tab.path_content.index + 1,
            self.tab.path_content.true_len() + 2
        )
    }

    fn first_row_flags(&self) -> String {
        let nb_flagged = self.status.flagged.len();
        let flag_string = if self.status.flagged.len() > 1 {
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

    fn default_preview_first_line(&self, tab: &Tab) -> Vec<String> {
        match tab.path_content.selected() {
            Some(fileinfo) => {
                let mut strings = vec![" Preview ".to_owned()];
                if !tab.preview.is_empty() {
                    strings.push(format!(" {} / {} ", tab.window.bottom, tab.preview.len()));
                };
                strings.push(format!(" {} ", fileinfo.path.to_string_lossy()));
                strings
            }
            None => vec!["".to_owned()],
        }
    }

    fn create_first_row(&self, tab: &Tab, disk_space: &str) -> Result<Vec<String>> {
        let first_row = match tab.mode {
            Mode::Normal | Mode::Tree => self.normal_first_row(disk_space)?,
            Mode::Preview => match &tab.preview {
                Preview::Text(text_content) => match text_content.kind {
                    TextKind::HELP => Self::help_first_row(),
                    TextKind::LOG => Self::log_first_row(),
                    _ => self.default_preview_first_line(tab),
                },
                _ => self.default_preview_first_line(tab),
            },
            _ => match self.tab.previous_mode {
                Mode::Normal | Mode::Tree => self.normal_first_row(disk_space)?,
                _ => vec![],
            },
        };
        Ok(first_row)
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Only the files around the selected one are displayed.
    /// We reverse the attributes of the selected one, underline the flagged files.
    /// When we display a simpler version, the second line is used to display the
    /// metadata of the selected file.
    fn files(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> Result<()> {
        let len = tab.path_content.content.len();
        let group_size: usize;
        let owner_size: usize;
        if status.display_full {
            group_size = tab.path_content.group_column_width();
            owner_size = tab.path_content.owner_column_width();
        } else {
            group_size = 0;
            owner_size = 0;
        }

        for (i, file) in tab
            .path_content
            .content
            .iter()
            .enumerate()
            .take(min(len, tab.window.bottom))
            .skip(tab.window.top)
        {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - tab.window.top;
            let mut attr = fileinfo_attr(file, self.colors);
            let string = if status.display_full {
                file.format(owner_size, group_size)?
            } else {
                file.format_simple()?
            };
            if status.flagged.contains(&file.path) {
                attr.effect |= Effect::BOLD | Effect::UNDERLINE;
            }
            canvas.print_with_attr(row, 0, &string, attr)?;
        }
        self.second_line(status, tab, canvas)?;
        if !self.attributes.has_window_below {
            self.log_line(canvas)?;
        }
        Ok(())
    }

    fn log_line(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let (_, height) = canvas.size()?;
        canvas.print_with_attr(height - 1, 4, &read_last_log_line(), ATTR_YELLOW_BOLD)?;
        Ok(())
    }

    fn tree(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> Result<()> {
        let left_margin = if status.display_full { 0 } else { 3 };
        let (_, height) = canvas.size()?;
        let (top, bottom, len) = tab.directory.calculate_tree_window(height);

        for (i, (metadata, prefix, colored_string)) in tab.directory.window(top, bottom, len) {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - top;
            let mut attr = colored_string.attr;
            if status.flagged.contains(&colored_string.path) {
                attr.effect |= Effect::BOLD | Effect::UNDERLINE;
            }
            let col_metadata = if status.display_full {
                canvas.print_with_attr(row, left_margin, &metadata.text, attr)?
            } else {
                0
            };
            let col_tree_prefix = canvas.print(row, left_margin + col_metadata, prefix)?;
            canvas.print_with_attr(
                row,
                left_margin + col_metadata + col_tree_prefix + 1,
                &colored_string.text,
                attr,
            )?;
        }
        self.second_line(status, tab, canvas)?;
        Ok(())
    }

    fn print_line_number(
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
    fn preview(&self, tab: &Tab, window: &ContentWindow, canvas: &mut dyn Canvas) -> Result<()> {
        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                for (i, vec_line) in (*syntaxed).window(window.top, window.bottom, length) {
                    let row_position = calc_line_row(i, window);
                    Self::print_line_number(row_position, i + 1, canvas)?;
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
                    line.print(canvas, row, line_number_width_hex + 1);
                }
            }
            Preview::Ueberzug(image) => {
                let (width, height) = canvas.size()?;
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
                        colored_string.attr,
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
            Preview::Pdf(text) => {
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

            Preview::Empty => (),
        }
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
        match self.tab.mode {
            Mode::Navigate(mode) => self.navigate(mode, canvas),
            Mode::NeedConfirmation(mode) => self.confirm(self.status, self.tab, mode, canvas),
            Mode::InputCompleted(_) => self.completion(self.tab, canvas),
            Mode::InputSimple(mode) => Self::display_static_lines(mode.lines(), canvas),
            _ => Ok(()),
        }?;
        self.cursor(self.tab, canvas)?;
        self.first_line(self.tab, canvas)?;
        Ok(())
    }
}

impl<'a> WinSecondary<'a> {
    const EDIT_BOX_OFFSET: usize = 9;
    const ATTR_YELLOW: Attr = color_to_attr(Color::YELLOW);
    const SORT_CURSOR_OFFSET: usize = 37;
    const PASSWORD_CURSOR_OFFSET: usize = 7;

    fn new(status: &'a Status, index: usize) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
        }
    }

    fn first_line(&self, tab: &Tab, canvas: &mut dyn Canvas) -> Result<()> {
        draw_colored_strings(0, 0, self.create_first_row(tab)?, canvas, false)
    }

    fn create_first_row(&self, tab: &Tab) -> Result<Vec<String>> {
        let first_row = match tab.mode {
            Mode::NeedConfirmation(confirmed_action) => {
                vec![format!("{confirmed_action}"), " (y/n)".to_owned()]
            }
            Mode::Navigate(Navigate::Marks(MarkAction::Jump)) => {
                vec!["Jump to...".to_owned()]
            }
            Mode::Navigate(Navigate::Marks(MarkAction::New)) => {
                vec!["Save mark...".to_owned()]
            }
            Mode::InputSimple(InputSimple::Password(password_kind, _encrypted_action, _)) => {
                info!("term: password");
                vec![format!("{password_kind}"), tab.input.password()]
            }
            Mode::InputCompleted(mode) => {
                let mut completion_strings = vec![tab.mode.to_string(), tab.input.string()];
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
                vec![tab.mode.to_string(), tab.input.string()]
            }
        };
        Ok(first_row)
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&self, tab: &Tab, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.set_cursor(0, tab.input.cursor_index + Self::EDIT_BOX_OFFSET)?;
        for (row, candidate) in tab.completion.proposals.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tab.completion.index {
                attr.effect |= Effect::REVERSE;
            }
            canvas.print_with_attr(row + ContentWindow::WINDOW_MARGIN_TOP, 4, candidate, attr)?;
        }
        Ok(())
    }

    fn display_static_lines(lines: &[&str], canvas: &mut dyn Canvas) -> Result<()> {
        for (row, line, attr) in enumerated_colored_iter!(lines) {
            canvas.print_with_attr(row + ContentWindow::WINDOW_MARGIN_TOP, 4, line, *attr)?;
        }
        Ok(())
    }

    /// Display a cursor in the top row, at a correct column.
    fn cursor(&self, tab: &Tab, canvas: &mut dyn Canvas) -> Result<()> {
        match tab.mode {
            Mode::Normal | Mode::Tree | Mode::Navigate(_) | Mode::Preview => {
                canvas.show_cursor(false)?;
            }
            Mode::InputSimple(InputSimple::Sort) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, Self::SORT_CURSOR_OFFSET)?;
            }
            Mode::InputSimple(InputSimple::Password(_, _, _)) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, Self::PASSWORD_CURSOR_OFFSET + tab.input.cursor_index)?;
            }
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, tab.input.cursor_index + Self::EDIT_BOX_OFFSET)?;
            }
            Mode::NeedConfirmation(confirmed_action) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, confirmed_action.cursor_offset())?;
            }
        }
        Ok(())
    }

    fn navigate(&self, navigable_mode: Navigate, canvas: &mut dyn Canvas) -> Result<()> {
        match navigable_mode {
            Navigate::Bulk => self.bulk(canvas, &self.status.bulk),
            Navigate::CliInfo => self.cli_info(self.status, canvas),
            Navigate::Compress => self.compress(canvas, &self.status.compression),
            Navigate::EncryptedDrive => self.encrypt(self.status, self.tab, canvas),
            Navigate::History => self.destination(canvas, &self.tab.history),
            Navigate::Jump => self.destination(canvas, &self.status.flagged),
            Navigate::Marks(_) => self.marks(self.status, canvas),
            Navigate::ShellMenu => self.shell_menu(self.status, canvas),
            Navigate::Shortcut => self.destination(canvas, &self.tab.shortcut),
            Navigate::Trash => self.trash(canvas, &self.status.trash),
        }
    }

    /// Display the possible destinations from a selectable content of PathBuf.
    fn destination(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<PathBuf>,
    ) -> Result<()> {
        canvas.print(0, 0, "Go to...")?;
        let content = &selectable.content();
        for (row, path, attr) in enumerated_colored_iter!(content) {
            let mut attr = *attr;
            if row == selectable.index() {
                attr.effect |= Effect::REVERSE;
            }
            let _ = canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().context("Unreadable filename")?,
                attr,
            );
        }
        Ok(())
    }

    fn bulk(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<String>,
    ) -> Result<()> {
        canvas.print(0, 0, "Action...")?;
        let content = &selectable.content();
        for (row, text, attr) in enumerated_colored_iter!(content) {
            let mut attr = *attr;
            if row == selectable.index() {
                attr.effect |= Effect::REVERSE;
            }
            let _ = canvas.print_with_attr(row + ContentWindow::WINDOW_MARGIN_TOP, 4, text, attr);
        }
        Ok(())
    }

    fn trash(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<TrashInfo>,
    ) -> Result<()> {
        canvas.print(
            1,
            0,
            "Enter: restore the selected file - x: delete permanently",
        )?;
        let content = &selectable.content();
        if content.is_empty() {
            let _ = canvas.print_with_attr(
                ContentWindow::WINDOW_MARGIN_TOP + 2,
                4,
                "Trash is empty",
                ATTR_YELLOW_BOLD,
            );
        } else {
            for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
                let mut attr = *attr;
                if row == selectable.index() {
                    attr.effect |= Effect::REVERSE;
                }
                let _ = canvas.print_with_attr(
                    row + ContentWindow::WINDOW_MARGIN_TOP,
                    4,
                    &format!("{trashinfo}"),
                    attr,
                );
            }
        }
        Ok(())
    }

    fn compress(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<CompressionMethod>,
    ) -> Result<()> {
        canvas.print_with_attr(
            1,
            0,
            "Archive and compress the flagged files. Pick a compression algorithm.",
            Self::ATTR_YELLOW,
        )?;
        let content = &selectable.content();
        for (row, compression_method, attr) in enumerated_colored_iter!(content) {
            let mut attr = *attr;
            if row == selectable.index() {
                attr.effect |= Effect::REVERSE;
            }

            let _ = canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                &format!("{compression_method}"),
                attr,
            );
        }
        Ok(())
    }

    fn marks(&self, status: &Status, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 1, "mark  path", Self::ATTR_YELLOW)?;

        for ((row, line), attr) in std::iter::zip(
            status.marks.as_strings().iter().enumerate(),
            MENU_COLORS.iter().cycle(),
        ) {
            let mut attr = *attr;
            if row == status.marks.index() {
                attr.effect |= Effect::REVERSE;
            }

            canvas.print_with_attr(row + 4, 3, line, attr)?;
        }
        Ok(())
    }

    fn shell_menu(&self, status: &Status, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 1, "pick a command", Self::ATTR_YELLOW)?;

        let tab = status.selected_non_mut();
        for ((row, (command, _)), attr) in std::iter::zip(
            status.shell_menu.content.iter().enumerate(),
            MENU_COLORS.iter().cycle(),
        ) {
            let mut attr = *attr;
            if row == status.shell_menu.index() {
                attr.effect |= Effect::REVERSE;
            }
            let row = calc_line_row(row, &tab.window) + 2;

            canvas.print_with_attr(row, 3, command, attr)?;
        }
        Ok(())
    }

    fn cli_info(&self, status: &Status, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 1, "pick a command", Self::ATTR_YELLOW)?;

        let tab = status.selected_non_mut();
        for ((row, command), attr) in std::iter::zip(
            status.cli_info.content.iter().enumerate(),
            MENU_COLORS.iter().cycle(),
        ) {
            let mut attr = *attr;
            if row == status.cli_info.index() {
                attr.effect |= Effect::REVERSE;
            }
            let row = calc_line_row(row, &tab.window) + 2;

            canvas.print_with_attr(row, 3, command, attr)?;
        }
        Ok(())
    }

    fn encrypt(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 3, "m: mount    --   u: unmount", Self::ATTR_YELLOW)?;
        for (i, device) in status.encrypted_devices.content.iter().enumerate() {
            let row = calc_line_row(i, &tab.window) + 2;
            let mut not_mounted_attr = Attr::default();
            let mut mounted_attr = Attr::from(Color::BLUE);
            if i == status.encrypted_devices.index() {
                not_mounted_attr.effect |= Effect::REVERSE;
                mounted_attr.effect |= Effect::REVERSE;
            }
            if status.encrypted_devices.content[i].is_mounted() {
                canvas.print_with_attr(row, 3, &device.as_string()?, mounted_attr)?;
            } else {
                canvas.print_with_attr(row, 3, &device.as_string()?, not_mounted_attr)?;
            }
        }
        Ok(())
    }

    /// Display a list of edited (deleted, copied, moved) files for confirmation
    fn confirm(
        &self,
        status: &Status,
        tab: &Tab,
        confirmed_mode: NeedConfirmation,
        canvas: &mut dyn Canvas,
    ) -> Result<()> {
        info!("confirmed action: {:?}", confirmed_mode);
        match confirmed_mode {
            NeedConfirmation::EmptyTrash => {
                for (row, trashinfo) in status.trash.content.iter().enumerate() {
                    canvas.print_with_attr(
                        row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                        4,
                        &format!("{trashinfo}"),
                        Attr::default(),
                    )?;
                }
            }
            _ => {
                for (row, path) in status.flagged.content.iter().enumerate() {
                    canvas.print_with_attr(
                        row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                        4,
                        path.to_str().context("Unreadable filename")?,
                        Attr::default(),
                    )?;
                }
            }
        }
        canvas.print_with_attr(
            2,
            3,
            &confirmed_mode.confirmation_string(&tab.path_content.path_to_str()),
            ATTR_YELLOW_BOLD,
        )?;

        Ok(())
    }
}

impl<'a> Widget for WinSecondary<'a> {}

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
    pub fn display_all(&mut self, status: &Status, colors: &Colors) -> Result<()> {
        self.hide_cursor()?;
        self.term.clear()?;

        let (width, _) = self.term.term_size()?;
        let disk_spaces = status.disk_spaces_per_tab();
        if status.dual_pane && width > MIN_WIDTH_FOR_DUAL_PANE {
            self.draw_dual_pane(status, &disk_spaces.0, &disk_spaces.1, colors)?
        } else {
            self.draw_single_pane(status, &disk_spaces.0, colors)?
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
        colors: &Colors,
    ) -> Result<()> {
        let (width, _) = self.term.term_size()?;
        let (first_selected, second_selected) = (status.index == 0, status.index == 1);
        let attributes_left = WinMainAttributes::new(
            0,
            false,
            first_selected,
            status.tabs[0].need_second_window(),
        );
        let win_main_left = WinMain::new(status, 0, disk_space_tab_0, colors, attributes_left);
        let attributes_right = WinMainAttributes::new(
            width / 2,
            true,
            second_selected,
            status.tabs[1].need_second_window(),
        );
        let win_main_right = WinMain::new(status, 1, disk_space_tab_1, colors, attributes_right);
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

    fn draw_single_pane(
        &mut self,
        status: &Status,
        disk_space_tab_0: &str,
        colors: &Colors,
    ) -> Result<()> {
        let attributes_left =
            WinMainAttributes::new(0, false, true, status.tabs[0].need_second_window());
        let win_main_left = WinMain::new(status, 0, disk_space_tab_0, colors, attributes_left);
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
    strings: Vec<String>,
    canvas: &mut dyn Canvas,
    reverse: bool,
) -> Result<()> {
    let mut col = 0;
    for (text, attr) in std::iter::zip(strings.iter(), FIRST_LINE_COLORS.iter().cycle()) {
        let mut attr = *attr;
        if reverse {
            attr.effect |= Effect::REVERSE;
        }
        col += canvas.print_with_attr(row, offset + col, text, attr)?;
    }
    Ok(())
}

fn calc_line_row(i: usize, window: &ContentWindow) -> usize {
    i + ContentWindow::WINDOW_MARGIN_TOP - window.top
}
