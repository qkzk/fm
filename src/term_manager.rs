use std::cmp::min;
use std::sync::Arc;

use image::Rgb;
use log::info;
use tuikit::attr::*;
use tuikit::event::Event;
use tuikit::prelude::*;
use tuikit::term::Term;

use crate::config::Colors;
use crate::constant_strings_paths::{HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE};
use crate::content_window::ContentWindow;
use crate::fileinfo::fileinfo_attr;
use crate::fm_error::{FmError, FmResult};
use crate::mode::{ConfirmedAction, InputKind, MarkAction, Mode};
use crate::preview::{Preview, TextKind, Window};
use crate::status::Status;
use crate::tab::Tab;

/// At least 100 chars width to display 2 tabs.
pub const MIN_WIDTH_FOR_DUAL_PANE: usize = 100;

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
    pub fn poll_event(&self) -> FmResult<Event> {
        Ok(self.term.poll_event()?)
    }
}

macro_rules! impl_preview {
    ($text:ident, $tab:ident, $length:ident, $canvas:ident, $line_number_width:ident) => {
        for (i, line) in (*$text).window($tab.window.top, $tab.window.bottom, $length) {
            let row = Self::calc_line_row(i, $tab);
            $canvas.print(row, $line_number_width + 3, line)?;
        }
    };
}

struct WinTab<'a> {
    status: &'a Status,
    tab: &'a Tab,
    disk_space: &'a str,
    colors: &'a Colors,
}

impl<'a> Draw for WinTab<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        match self.tab.mode {
            Mode::Jump => self.jump_list(self.status, canvas),
            Mode::History => self.history(self.tab, canvas),
            Mode::InputCompleted(_) => self.completion(self.tab, canvas),
            Mode::NeedConfirmation(confirmed_mode) => {
                self.confirmation(self.status, self.tab, confirmed_mode, canvas)
            }
            Mode::Preview => self.preview(self.tab, canvas),
            Mode::Shortcut => self.shortcuts(self.tab, canvas),
            Mode::InputSimple(InputKind::Marks(_)) => self.marks(self.status, self.tab, canvas),
            _ => self.files(self.status, self.tab, canvas),
        }?;
        self.cursor(self.tab, canvas)?;
        self.first_line(self.tab, self.disk_space, canvas)?;
        Ok(())
    }
}

impl<'a> Widget for WinTab<'a> {}

impl<'a> WinTab<'a> {
    const EDIT_BOX_OFFSET: usize = 9;
    const SORT_CURSOR_OFFSET: usize = 37;
    const ATTR_LINE_NR: Attr = color_to_attr(Color::CYAN);
    const ATTR_YELLOW: Attr = color_to_attr(Color::YELLOW);
    const ATTR_YELLOW_BOLD: Attr = Attr {
        fg: Color::YELLOW,
        bg: Color::Default,
        effect: Effect::BOLD,
    };
    const LINE_COLORS: [Attr; 6] = [
        color_to_attr(Color::Rgb(231, 162, 156)),
        color_to_attr(Color::Rgb(144, 172, 186)),
        color_to_attr(Color::Rgb(214, 125, 83)),
        color_to_attr(Color::Rgb(91, 152, 119)),
        color_to_attr(Color::Rgb(152, 87, 137)),
        color_to_attr(Color::Rgb(230, 189, 87)),
    ];
    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    fn first_line(&self, tab: &Tab, disk_space: &str, canvas: &mut dyn Canvas) -> FmResult<()> {
        let first_row = self.create_first_row(tab, disk_space)?;
        self.draw_colored_strings(0, 0, first_row, canvas)?;
        Ok(())
    }

    fn second_line(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        match tab.mode {
            Mode::Normal => {
                if !status.display_full {
                    if let Some(file) = tab.path_content.selected_file() {
                        let owner_size = file.owner.len();
                        let group_size = file.group.len();
                        let mut attr = fileinfo_attr(status, file, self.colors);
                        attr.effect ^= Effect::REVERSE;
                        canvas.print_with_attr(
                            1,
                            0,
                            &file.format(owner_size, group_size)?,
                            attr,
                        )?;
                    }
                }
            }
            Mode::InputSimple(InputKind::Filter) => {
                canvas.print_with_attr(
                    1,
                    0,
                    "by name: n name, by ext: e ext, only directories: d, reset: a",
                    Self::ATTR_YELLOW_BOLD,
                )?;
            }
            _ => (),
        }

        Ok(())
    }

    fn create_first_row(&self, tab: &Tab, disk_space: &str) -> FmResult<Vec<String>> {
        let first_row = match tab.mode {
            Mode::Normal => {
                vec![
                    format!("{} ", tab.path_content.path_to_str()?),
                    format!("{} files ", tab.path_content.files.len()),
                    format!("{}  ", tab.path_content.used_space()),
                    format!("Avail: {}  ", disk_space),
                    format!("{}  ", &tab.path_content.git_string()?),
                ]
            }
            Mode::NeedConfirmation(confirmed_action) => {
                vec![format!("{} (y/n)", confirmed_action)]
            }
            Mode::Preview => match &tab.preview {
                Preview::Text(text_content) => {
                    if matches!(text_content.kind, TextKind::HELP) {
                        vec![
                            HELP_FIRST_SENTENCE.to_owned(),
                            HELP_SECOND_SENTENCE.to_owned(),
                        ]
                    } else {
                        Self::default_preview_first_line(tab)
                    }
                }
                _ => Self::default_preview_first_line(tab),
            },
            Mode::InputSimple(InputKind::Marks(MarkAction::Jump)) => vec!["Jump to...".to_owned()],
            Mode::InputSimple(InputKind::Marks(MarkAction::New)) => vec!["Save mark...".to_owned()],
            _ => {
                vec![
                    format!("{}", tab.mode.clone()),
                    format!("{}", tab.input.string()),
                ]
            }
        };
        Ok(first_row)
    }

    fn default_preview_first_line(tab: &Tab) -> Vec<String> {
        match tab.path_content.selected_file() {
            Some(fileinfo) => {
                vec![
                    format!("{}", tab.mode.clone()),
                    format!("{}", fileinfo.path.to_string_lossy()),
                ]
            }
            None => vec!["".to_owned()],
        }
    }

    fn draw_colored_strings(
        &self,
        row: usize,
        offset: usize,
        strings: Vec<String>,
        canvas: &mut dyn Canvas,
    ) -> FmResult<()> {
        let mut col = 0;
        for (text, attr) in std::iter::zip(strings.iter(), Self::LINE_COLORS.iter().cycle()) {
            canvas.print_with_attr(row, offset + col, text, *attr)?;
            col += text.len()
        }
        Ok(())
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Those files are always shown, which make it a little bit faster in;
    /// normal (ie. default) mode.
    /// When there's too much files, only those around the selected one are
    /// displayed.
    fn files(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        let len = tab.path_content.files.len();
        for (i, (file, string)) in std::iter::zip(
            tab.path_content.files.iter(),
            tab.path_content.strings(status.display_full).iter(),
        )
        .enumerate()
        .take(min(len, tab.window.bottom + 1))
        .skip(tab.window.top)
        {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - tab.window.top;
            let mut attr = fileinfo_attr(status, file, self.colors);
            if status.flagged.contains(&file.path) {
                attr.effect |= Effect::BOLD | Effect::UNDERLINE;
            }
            canvas.print_with_attr(row, 0, string, attr)?;
        }
        self.second_line(status, tab, canvas)?;
        Ok(())
    }

    /// Display a cursor in the top row, at a correct column.
    fn cursor(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        match tab.mode {
            Mode::Normal
            | Mode::InputSimple(InputKind::Marks(_))
            | Mode::Preview
            | Mode::Shortcut
            | Mode::Jump
            | Mode::History => {
                canvas.show_cursor(false)?;
            }
            Mode::InputSimple(InputKind::Sort) => {
                canvas.set_cursor(0, Self::SORT_CURSOR_OFFSET)?;
            }
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, tab.input.cursor_index + Self::EDIT_BOX_OFFSET)?;
            }
            Mode::NeedConfirmation(confirmed_action) => {
                canvas.set_cursor(0, confirmed_action.cursor_offset())?;
            }
        }
        Ok(())
    }

    /// Display the possible jump destination from flagged files.
    fn jump_list(&self, tabs: &Status, canvas: &mut dyn Canvas) -> FmResult<()> {
        canvas.print(0, 0, "Jump to...")?;
        for (row, path) in tabs.flagged.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tabs.jump_index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::custom("display", "Unreadable filename"))?,
                attr,
            );
        }
        Ok(())
    }

    /// Display the history of visited directories.
    fn history(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        canvas.print(0, 0, "Go to...")?;
        for (row, path) in tab.history.visited.iter().rev().enumerate() {
            let mut attr = Attr::default();
            if tab.history.len() > tab.history.index
                && row == tab.history.len() - tab.history.index - 1
            {
                attr.effect |= Effect::REVERSE;
            }
            canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::custom("display", "Unreadable filename"))?,
                attr,
            )?;
        }
        Ok(())
    }

    /// Display the predefined shortcuts.
    fn shortcuts(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        canvas.print(0, 0, "Go to...")?;
        for (row, path) in tab.shortcut.shortcuts.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tab.shortcut.index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::custom("display", "Unreadable filename"))?,
                attr,
            );
        }
        Ok(())
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
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

    /// Display a list of edited (deleted, copied, moved) files for confirmation
    fn confirmation(
        &self,
        status: &Status,
        tab: &Tab,
        confirmed_mode: ConfirmedAction,
        canvas: &mut dyn Canvas,
    ) -> FmResult<()> {
        for (row, path) in status.flagged.iter().enumerate() {
            canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::custom("display", "Unreadable filename"))?,
                Attr::default(),
            )?;
        }
        info!("confirmed action: {:?}", confirmed_mode);
        let content = match confirmed_mode {
            ConfirmedAction::Copy => {
                format!(
                    "Files will be copied to {}",
                    tab.path_content.path_to_str()?
                )
            }
            ConfirmedAction::Delete => "Files will deleted permanently".to_owned(),
            ConfirmedAction::Move => {
                format!("Files will be moved to {}", tab.path_content.path_to_str()?)
            }
        };
        canvas.print_with_attr(2, 3, &content, Self::ATTR_YELLOW_BOLD)?;

        Ok(())
    }

    fn preview_line_numbers(
        &self,
        tab: &Tab,
        line_index: usize,
        row_index: usize,
        canvas: &mut dyn Canvas,
    ) -> FmResult<usize> {
        Ok(canvas.print_with_attr(
            row_index,
            0,
            &(line_index + 1 + tab.window.top).to_string(),
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
    fn preview(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                for (i, vec_line) in (*syntaxed).window(tab.window.top, tab.window.bottom, length) {
                    let row = Self::calc_line_row(i, tab);
                    self.preview_line_numbers(tab, i, row, canvas)?;
                    for token in vec_line.iter() {
                        //TODO! fix token print
                        token.print(canvas, row, line_number_width)?;
                    }
                }
            }
            Preview::Binary(bin) => {
                let line_number_width_hex = format!("{:x}", bin.len() * 16).len();

                for (i, line) in (*bin).window(tab.window.top, tab.window.bottom, length) {
                    let row = Self::calc_line_row(i, tab);

                    canvas.print_with_attr(
                        row,
                        0,
                        &format_line_nr_hex(i + 1 + tab.window.top, line_number_width_hex),
                        Self::ATTR_LINE_NR,
                    )?;
                    //TODO! Fix line print
                    line.print(canvas, row, line_number_width_hex + 1);
                }
            }
            Preview::Archive(text) => {
                for (i, line) in (*text).window(tab.window.top, tab.window.bottom, length) {
                    let row = Self::calc_line_row(i, tab);
                    canvas.print_with_attr(
                        row,
                        0,
                        &(i + 1 + tab.window.top).to_string(),
                        Self::ATTR_LINE_NR,
                    )?;
                    canvas.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Thumbnail(image) => {
                let (width, height) = canvas.size()?;

                if let Ok(scaled_image) = (*image).resized_rgb8(width as u32 / 2, height as u32 - 3)
                {
                    let (width, _) = scaled_image.dimensions();
                    for (i, pixel) in scaled_image.pixels().enumerate() {
                        let (r, g, b) = pixel_values(pixel);
                        let (row, col) = pixel_position(i, width);
                        print_pixel(canvas, row, col, r, g, b)?;
                    }
                } else {
                    canvas.print(
                        3,
                        3,
                        &format!("Not a displayable image: {:?}", image.img_path),
                    )?;
                }
            }
            Preview::Text(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Pdf(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Exif(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Media(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Empty => (),
        }
        Ok(())
    }

    fn marks(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        canvas.print_with_attr(2, 1, "mark  path", Self::ATTR_YELLOW)?;

        for (i, line) in status.marks.as_strings().iter().enumerate() {
            let row = Self::calc_line_row(i, tab) + 2;
            canvas.print(row, 3, line)?;
        }
        Ok(())
    }

    fn calc_line_row(i: usize, status: &Tab) -> usize {
        i + ContentWindow::WINDOW_MARGIN_TOP - status.window.top
    }
}

/// Is responsible for displaying content in the terminal.
/// It uses an already created terminal.
pub struct Display {
    /// The Tuikit terminal attached to the display.
    /// It will print every symbol shown on screen.
    term: Arc<Term>,
    colors: Colors,
}

impl Display {
    const SELECTED_BORDER: Attr = color_to_attr(Color::LIGHT_BLUE);
    const INERT_BORDER: Attr = color_to_attr(Color::Default);

    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Arc<Term>, colors: Colors) -> Self {
        Self { term, colors }
    }

    /// Used to force a display of the cursor before leaving the application.
    /// Most of the times we don't need a cursor and it's hidden. We have to
    /// do it unless the shell won't display a cursor anymore.
    pub fn show_cursor(&self) -> FmResult<()> {
        Ok(self.term.show_cursor(true)?)
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
    pub fn display_all(&mut self, status: &Status) -> FmResult<()> {
        self.term.clear()?;

        let (width, _) = self.term.term_size()?;
        let disk_spaces = status.disk_spaces_per_tab();
        if status.dual_pane && width > MIN_WIDTH_FOR_DUAL_PANE {
            self.draw_dual_pane(status, disk_spaces.0, disk_spaces.1)?
        } else {
            self.draw_single_pane(status, disk_spaces.0)?
        }

        Ok(self.term.present()?)
    }

    fn draw_dual_pane(
        &mut self,
        status: &Status,
        disk_space_tab_0: String,
        disk_space_tab_1: String,
    ) -> FmResult<()> {
        let win_left = WinTab {
            status,
            tab: &status.tabs[0],
            disk_space: &disk_space_tab_0,
            colors: &self.colors,
        };
        let win_right = WinTab {
            status,
            tab: &status.tabs[1],
            disk_space: &disk_space_tab_1,
            colors: &self.colors,
        };
        let (left_border, right_border) = if status.index == 0 {
            (Self::SELECTED_BORDER, Self::INERT_BORDER)
        } else {
            (Self::INERT_BORDER, Self::SELECTED_BORDER)
        };
        let hsplit = HSplit::default()
            .split(Win::new(&win_left).border(true).border_attr(left_border))
            .split(Win::new(&win_right).border(true).border_attr(right_border));
        Ok(self.term.draw(&hsplit)?)
    }

    fn draw_single_pane(&mut self, status: &Status, disk_space_tab_0: String) -> FmResult<()> {
        let win_left = WinTab {
            status,
            tab: &status.tabs[0],
            disk_space: &disk_space_tab_0,
            colors: &self.colors,
        };
        let win = Win::new(&win_left)
            .border(true)
            .border_attr(Self::SELECTED_BORDER);
        Ok(self.term.draw(&win)?)
    }

    /// Reads and returns the `tuikit::term::Term` height.
    pub fn height(&self) -> FmResult<usize> {
        let (_, height) = self.term.term_size()?;
        Ok(height)
    }
}

fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
    format!("{:0width$x}", line_nr)
}

const fn color_to_attr(color: Color) -> Attr {
    Attr {
        fg: color,
        bg: Color::Default,
        effect: Effect::empty(),
    }
}

const fn pixel_values(pixel: &Rgb<u8>) -> (u8, u8, u8) {
    let [r, g, b] = pixel.0;
    (r, g, b)
}

const fn pixel_position(i: usize, width: u32) -> (usize, usize) {
    let col = 2 * (i % width as usize);
    let row = i / width as usize + 3;
    (row, col)
}

fn print_pixel(
    canvas: &mut dyn Canvas,
    row: usize,
    col: usize,
    r: u8,
    g: u8,
    b: u8,
) -> FmResult<()> {
    canvas.print_with_attr(
        row,
        col,
        "██",
        Attr {
            fg: Color::Rgb(r, g, b),
            bg: Color::Rgb(r, g, b),
            ..Default::default()
        },
    )?;
    Ok(())
}
