use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use image::Rgb;
use log::info;
use tuikit::attr::*;
use tuikit::event::Event;
use tuikit::prelude::*;
use tuikit::term::Term;

use crate::constant_strings_paths::{
    FILTER_PRESENTATION, HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE,
};
use crate::content_window::ContentWindow;
use crate::fileinfo::{fileinfo_attr, FileInfo};
use crate::fm_error::{FmError, FmResult};
use crate::mode::{InputSimple, MarkAction, Mode, Navigate, NeedConfirmation};
use crate::preview::{Preview, TextKind, Window};
use crate::selectable_content::SelectableContent;
use crate::status::Status;
use crate::tab::Tab;
use crate::trash::TrashInfo;

/// At least 100 chars width to display 2 tabs.
pub const MIN_WIDTH_FOR_DUAL_PANE: usize = 100;

const FIRST_LINE_COLORS: [Attr; 6] = [
    color_to_attr(Color::Rgb(231, 162, 156)),
    color_to_attr(Color::Rgb(144, 172, 186)),
    color_to_attr(Color::Rgb(214, 125, 83)),
    color_to_attr(Color::Rgb(91, 152, 119)),
    color_to_attr(Color::Rgb(152, 87, 137)),
    color_to_attr(Color::Rgb(230, 189, 87)),
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
    pub fn poll_event(&self) -> FmResult<Event> {
        Ok(self.term.poll_event()?)
    }
}

macro_rules! impl_preview {
    ($text:ident, $tab:ident, $length:ident, $canvas:ident, $line_number_width:ident) => {
        for (i, line) in (*$text).window($tab.window.top, $tab.window.bottom, $length) {
            let row = calc_line_row(i, $tab);
            $canvas.print(row, $line_number_width + 3, line)?;
        }
    };
}

struct WinMain<'a> {
    status: &'a Status,
    tab: &'a Tab,
    disk_space: &'a str,
}

impl<'a> Draw for WinMain<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        canvas.clear()?;
        match self.tab.mode {
            Mode::Preview => self.preview(self.tab, canvas),
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

    fn new(status: &'a Status, index: usize, disk_space: &'a str) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
            disk_space,
        }
    }

    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    fn first_line(&self, tab: &Tab, disk_space: &str, canvas: &mut dyn Canvas) -> FmResult<()> {
        draw_colored_strings(0, 0, self.create_first_row(tab, disk_space)?, canvas)
    }

    fn second_line(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        match tab.mode {
            Mode::Normal => {
                if !status.display_full {
                    if let Some(file) = tab.selected() {
                        self.second_line_detailed(file, status, canvas)?;
                    }
                } else {
                    self.second_line_simple(status, canvas)?;
                }
            }
            Mode::Tree => {
                if let Some(file) = tab.selected() {
                    self.second_line_detailed(file, status, canvas)?;
                }
            }
            Mode::InputSimple(InputSimple::Filter) => {
                canvas.print_with_attr(1, 0, FILTER_PRESENTATION, ATTR_YELLOW_BOLD)?;
            }
            _ => (),
        }

        Ok(())
    }

    fn second_line_detailed(
        &self,
        file: &FileInfo,
        status: &Status,
        canvas: &mut dyn Canvas,
    ) -> FmResult<usize> {
        let owner_size = file.owner.len();
        let group_size = file.group.len();
        let mut attr = fileinfo_attr(file, &status.config_colors);
        attr.effect ^= Effect::REVERSE;
        Ok(canvas.print_with_attr(1, 0, &file.format(owner_size, group_size)?, attr)?)
    }

    fn second_line_simple(&self, status: &Status, canvas: &mut dyn Canvas) -> FmResult<usize> {
        Ok(canvas.print_with_attr(
            1,
            0,
            &format!("{}", &status.selected_non_mut().filter),
            ATTR_YELLOW_BOLD,
        )?)
    }

    fn create_first_row(&self, tab: &Tab, disk_space: &str) -> FmResult<Vec<String>> {
        let first_row = match tab.mode {
            Mode::Normal | Mode::Tree => {
                vec![
                    format!("{} ", tab.path_content.path.display()),
                    format!("{} files ", tab.path_content.true_len()),
                    format!("{}  ", tab.path_content.used_space()),
                    format!("Avail: {}  ", disk_space),
                    format!("{}  ", &tab.path_content.git_string()?),
                ]
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
            _ => match tab.previous_mode {
                Mode::Normal | Mode::Tree => {
                    vec![
                        format!("{} ", tab.path_content.path.display()),
                        format!("{} files ", tab.path_content.true_len()),
                        format!("{}  ", tab.path_content.used_space()),
                        format!("Avail: {}  ", disk_space),
                        format!("{}  ", &tab.path_content.git_string()?),
                    ]
                }
                _ => vec![],
            },
        };
        Ok(first_row)
    }

    fn default_preview_first_line(tab: &Tab) -> Vec<String> {
        match tab.path_content.selected() {
            Some(fileinfo) => {
                let mut strings = vec![
                    format!("{}", tab.mode.clone()),
                    format!("{}", fileinfo.path.to_string_lossy()),
                ];
                if !tab.preview.is_empty() {
                    strings.push(format!(" {} / {}", tab.window.bottom, tab.preview.len()));
                };
                strings
            }
            None => vec!["".to_owned()],
        }
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Only the files around the selected one are displayed.
    /// We reverse the attributes of the selected one, underline the flagged files.
    /// When we display a simpler version, the second line is used to display the
    /// metadata of the selected file.
    fn files(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        let len = tab.path_content.content.len();
        for (i, (file, string)) in std::iter::zip(
            tab.path_content.content.iter(),
            tab.path_content.strings(status.display_full).iter(),
        )
        .enumerate()
        .take(min(len, tab.window.bottom + 1))
        .skip(tab.window.top)
        {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - tab.window.top;
            let mut attr = fileinfo_attr(file, &status.config_colors);
            if status.flagged.contains(&file.path) {
                attr.effect |= Effect::BOLD | Effect::UNDERLINE;
            }
            canvas.print_with_attr(row, 0, string, attr)?;
        }
        self.second_line(status, tab, canvas)?;
        Ok(())
    }

    fn tree(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        let line_number_width = 3;

        let (_, height) = canvas.size()?;
        let (top, bottom, len) = tab.directory.calculate_tree_window(height);
        for (i, (prefix, colored_string)) in tab.directory.window(top, bottom, len) {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - top;
            let col = canvas.print(row, line_number_width, prefix)?;
            let mut attr = colored_string.attr;
            if status.flagged.contains(&colored_string.path) {
                attr.effect |= Effect::BOLD | Effect::UNDERLINE;
            }
            canvas.print_with_attr(row, line_number_width + col + 1, &colored_string.text, attr)?;
        }
        self.second_line(status, tab, canvas)?;
        Ok(())
    }

    fn print_line_number(
        row_position_in_canvas: usize,
        line_number_to_print: usize,
        canvas: &mut dyn Canvas,
    ) -> FmResult<usize> {
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
    fn preview(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                for (i, vec_line) in (*syntaxed).window(tab.window.top, tab.window.bottom, length) {
                    let row_position = calc_line_row(i, tab);
                    Self::print_line_number(row_position, i + 1, canvas)?;
                    for token in vec_line.iter() {
                        token.print(canvas, row_position, line_number_width)?;
                    }
                }
            }
            Preview::Binary(bin) => {
                let line_number_width_hex = format!("{:x}", bin.len() * 16).len();

                for (i, line) in (*bin).window(tab.window.top, tab.window.bottom, length) {
                    let row = calc_line_row(i, tab);

                    canvas.print_with_attr(
                        row,
                        0,
                        &format_line_nr_hex(i + 1 + tab.window.top, line_number_width_hex),
                        Self::ATTR_LINE_NR,
                    )?;
                    line.print(canvas, row, line_number_width_hex + 1);
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
            Preview::Directory(directory) => {
                for (i, (prefix, colored_string)) in
                    (directory).window(tab.window.top, tab.window.bottom, length)
                {
                    let row = calc_line_row(i, tab);
                    let col = canvas.print(row, line_number_width, prefix)?;
                    canvas.print_with_attr(
                        row,
                        line_number_width + col + 1,
                        &colored_string.text,
                        colored_string.attr,
                    )?;
                }
            }
            Preview::Archive(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Exif(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Media(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Pdf(text) => impl_preview!(text, tab, length, canvas, line_number_width),
            Preview::Text(text) => impl_preview!(text, tab, length, canvas, line_number_width),

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
            Mode::Navigate(Navigate::Jump) => self.destination(canvas, &self.status.flagged),
            Mode::Navigate(Navigate::History) => self.destination(canvas, &self.tab.history),
            Mode::Navigate(Navigate::Shortcut) => self.destination(canvas, &self.tab.shortcut),
            Mode::Navigate(Navigate::Trash) => self.trash(canvas, &self.status.trash),
            Mode::NeedConfirmation(confirmed_mode) => {
                self.confirmation(self.status, self.tab, confirmed_mode, canvas)
            }
            Mode::InputCompleted(_) => self.completion(self.tab, canvas),
            Mode::InputSimple(InputSimple::Marks(_)) => self.marks(self.status, self.tab, canvas),
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

    fn new(status: &'a Status, index: usize) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
        }
    }

    fn first_line(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        draw_colored_strings(0, 0, self.create_first_row(tab)?, canvas)
    }

    fn create_first_row(&self, tab: &Tab) -> FmResult<Vec<String>> {
        let first_row = match tab.mode {
            Mode::NeedConfirmation(confirmed_action) => {
                vec![format!("{} (y/n)", confirmed_action)]
            }
            Mode::InputSimple(InputSimple::Marks(MarkAction::Jump)) => {
                vec!["Jump to...".to_owned()]
            }
            Mode::InputSimple(InputSimple::Marks(MarkAction::New)) => {
                vec!["Save mark...".to_owned()]
            }
            _ => {
                vec![
                    format!("{}", tab.mode.clone()),
                    format!("{}", tab.input.string()),
                ]
            }
        };
        Ok(first_row)
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
    /// Display a cursor in the top row, at a correct column.
    fn cursor(&self, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        match tab.mode {
            Mode::Normal
            | Mode::Tree
            | Mode::InputSimple(InputSimple::Marks(_))
            | Mode::Navigate(_)
            | Mode::Preview => {
                canvas.show_cursor(false)?;
            }
            Mode::InputSimple(InputSimple::Sort) => {
                canvas.show_cursor(true)?;
                canvas.set_cursor(0, Self::SORT_CURSOR_OFFSET)?;
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

    /// Display the possible destinations from a selectable content of PathBuf.
    fn destination(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<PathBuf>,
    ) -> FmResult<()> {
        canvas.print(0, 0, "Go to...")?;
        for (row, path) in selectable.content().iter().enumerate() {
            let mut attr = Attr::default();
            if row == selectable.index() {
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

    fn trash(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<TrashInfo>,
    ) -> FmResult<()> {
        canvas.print(1, 0, "Restore the selected file")?;
        for (row, trashinfo) in selectable.content().iter().enumerate() {
            let mut attr = Attr::default();
            if row == selectable.index() {
                attr.effect |= Effect::REVERSE;
            }

            let _ = canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                &format!("{}", trashinfo),
                attr,
            );
        }
        Ok(())
    }

    fn marks(&self, status: &Status, tab: &Tab, canvas: &mut dyn Canvas) -> FmResult<()> {
        canvas.print_with_attr(2, 1, "mark  path", Self::ATTR_YELLOW)?;

        for (i, line) in status.marks.as_strings().iter().enumerate() {
            let row = calc_line_row(i, tab) + 2;
            canvas.print(row, 3, line)?;
        }
        Ok(())
    }

    /// Display a list of edited (deleted, copied, moved) files for confirmation
    fn confirmation(
        &self,
        status: &Status,
        tab: &Tab,
        confirmed_mode: NeedConfirmation,
        canvas: &mut dyn Canvas,
    ) -> FmResult<()> {
        info!("confirmed action: {:?}", confirmed_mode);
        match confirmed_mode {
            NeedConfirmation::EmptyTrash => {
                for (row, trashinfo) in status.trash.content.iter().enumerate() {
                    canvas.print_with_attr(
                        row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                        4,
                        &format!("{}", trashinfo),
                        Attr::default(),
                    )?;
                }
            }
            NeedConfirmation::Copy | NeedConfirmation::Delete | NeedConfirmation::Move => {
                for (row, path) in status.flagged.content.iter().enumerate() {
                    canvas.print_with_attr(
                        row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                        4,
                        path.to_str()
                            .ok_or_else(|| FmError::custom("display", "Unreadable filename"))?,
                        Attr::default(),
                    )?;
                }
            }
        }
        let confirmation_string = match confirmed_mode {
            NeedConfirmation::Copy => {
                format!(
                    "Files will be copied to {}",
                    tab.path_content.path_to_str()?
                )
            }
            NeedConfirmation::Delete => "Files will deleted permanently".to_owned(),
            NeedConfirmation::Move => {
                format!("Files will be moved to {}", tab.path_content.path_to_str()?)
            }
            NeedConfirmation::EmptyTrash => "Trash will be emptied".to_owned(),
        };
        canvas.print_with_attr(2, 3, &confirmation_string, ATTR_YELLOW_BOLD)?;

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
    const MAX_PERCENT_SECOND_WINDOW: usize = 50;

    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    /// Used to force a display of the cursor before leaving the application.
    /// Most of the times we don't need a cursor and it's hidden. We have to
    /// do it unless the shell won't display a cursor anymore.
    pub fn show_cursor(&self) -> FmResult<()> {
        Ok(self.term.show_cursor(true)?)
    }

    fn hide_cursor(&self) -> FmResult<()> {
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
    pub fn display_all(&mut self, status: &Status) -> FmResult<()> {
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

    fn percent_for_second_window(tab: &Tab) -> usize {
        if tab.need_second_window() {
            Self::MAX_PERCENT_SECOND_WINDOW
        } else {
            0
        }
    }

    fn vertical_split<'a>(
        win_main: &'a WinMain,
        win_secondary: &'a WinSecondary,
        border: Attr,
        percent: usize,
    ) -> VSplit<'a> {
        VSplit::default()
            .split(
                Win::new(win_main)
                    .basis(Size::Percent(100 - percent))
                    .shrink(4)
                    .border(true)
                    .border_attr(border),
            )
            .split(
                Win::new(win_secondary)
                    .basis(Size::Percent(percent))
                    .shrink(0)
                    .border(true)
                    .border_attr(border),
            )
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
    ) -> FmResult<()> {
        let win_main_left = WinMain::new(status, 0, disk_space_tab_0);
        let win_main_right = WinMain::new(status, 1, disk_space_tab_1);
        let win_second_left = WinSecondary::new(status, 0);
        let win_second_right = WinSecondary::new(status, 1);
        let (border_left, border_right) = self.borders(status);
        let percent_left = Self::percent_for_second_window(&status.tabs[0]);
        let percent_right = Self::percent_for_second_window(&status.tabs[1]);
        let hsplit = HSplit::default()
            .split(Self::vertical_split(
                &win_main_left,
                &win_second_left,
                border_left,
                percent_left,
            ))
            .split(Self::vertical_split(
                &win_main_right,
                &win_second_right,
                border_right,
                percent_right,
            ));
        Ok(self.term.draw(&hsplit)?)
    }

    fn draw_single_pane(&mut self, status: &Status, disk_space_tab_0: &str) -> FmResult<()> {
        let win_main_left = WinMain::new(status, 0, disk_space_tab_0);
        let win_second_left = WinSecondary::new(status, 0);
        let percent_left = Self::percent_for_second_window(&status.tabs[0]);
        let win = Self::vertical_split(
            &win_main_left,
            &win_second_left,
            Self::SELECTED_BORDER,
            percent_left,
        );
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

fn draw_colored_strings(
    row: usize,
    offset: usize,
    strings: Vec<String>,
    canvas: &mut dyn Canvas,
) -> FmResult<()> {
    let mut col = 0;
    for (text, attr) in std::iter::zip(strings.iter(), FIRST_LINE_COLORS.iter().cycle()) {
        col += canvas.print_with_attr(row, offset + col, text, *attr)?;
    }
    Ok(())
}

fn calc_line_row(i: usize, tab: &Tab) -> usize {
    i + ContentWindow::WINDOW_MARGIN_TOP - tab.window.top
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
