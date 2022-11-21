use std::cmp::min;
use std::sync::Arc;

use log::info;
use tuikit::attr::*;
use tuikit::event::Event;
use tuikit::term::Term;

use crate::config::Colors;
use crate::content_window::ContentWindow;
use crate::fileinfo::fileinfo_attr;
use crate::fm_error::{FmError, FmResult};
use crate::last_edition::LastEdition;
use crate::mode::{MarkAction, Mode};
use crate::preview::Preview;
use crate::status::Status;
use crate::tab::Tab;

pub struct EventReader {
    term: Arc<Term>,
}

impl EventReader {
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    pub fn poll_event(&self) -> FmResult<Event> {
        Ok(self.term.poll_event()?)
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
    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Arc<Term>, colors: Colors) -> Self {
        Self { term, colors }
    }

    pub fn show_cursor(&self) -> FmResult<()> {
        Ok(self.term.show_cursor(true)?)
    }

    const EDIT_BOX_OFFSET: usize = 9;
    const SORT_CURSOR_OFFSET: usize = 37;
    const ATTR_LINE_NR: Attr = color_to_attr(Color::CYAN);
    const ATTR_YELLOW: Attr = color_to_attr(Color::YELLOW);
    const LINE_COLORS: [Attr; 6] = [
        color_to_attr(Color::Rgb(231, 162, 156)),
        color_to_attr(Color::Rgb(144, 172, 186)),
        color_to_attr(Color::Rgb(214, 125, 83)),
        color_to_attr(Color::Rgb(91, 152, 119)),
        color_to_attr(Color::Rgb(152, 87, 137)),
        color_to_attr(Color::Rgb(230, 189, 87)),
    ];
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
    pub fn display_all(&mut self, status: &Status, disk_space: String) -> FmResult<()> {
        self.term.clear()?;
        match status.selected_non_mut().mode {
            Mode::Jump => self.jump_list(status),
            Mode::History => self.history(status),
            Mode::Exec | Mode::Goto | Mode::Search => self.completion(status),
            Mode::NeedConfirmation => self.confirmation(status),
            Mode::Preview | Mode::Help => self.preview(status),
            Mode::Shortcut => self.shortcuts(status),
            Mode::Marks(MarkAction::New) | Mode::Marks(MarkAction::Jump) => self.marks(status),
            _ => self.files(status),
        }?;
        self.cursor(status)?;
        self.first_line(status, disk_space)?;
        Ok(self.term.present()?)
    }

    /// Reads and returns the `tuikit::term::Term` height.
    pub fn height(&self) -> FmResult<usize> {
        let (_, height) = self.term.term_size()?;
        Ok(height)
    }

    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    fn first_line(&mut self, status: &Status, disk_space: String) -> FmResult<()> {
        let mut offset = 0;
        if let Mode::Normal = status.selected_non_mut().mode {
            offset = self.tab_bar(status)?
        }
        let first_row = self.create_first_row(status, disk_space)?;
        self.draw_colored_strings(offset, first_row)?;
        Ok(())
    }

    fn create_first_row(&mut self, status: &Status, disk_space: String) -> FmResult<Vec<String>> {
        let tab = status.selected_non_mut();
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
            Mode::NeedConfirmation => {
                vec![
                    format!("Confirm {}", tab.last_edition),
                    "(y/n) : ".to_owned(),
                ]
            }
            Mode::Preview => match tab.path_content.selected_file() {
                Some(fileinfo) => {
                    vec![
                        format!("{:?}", tab.mode.clone()),
                        format!("{}", fileinfo.path.to_string_lossy()),
                    ]
                }
                None => vec!["".to_owned()],
            },
            Mode::Help => vec![
                "fm: a dired like file manager.".to_owned(),
                "Default keybindings.".to_owned(),
            ],
            Mode::Marks(MarkAction::Jump) => vec!["Jump to...".to_owned()],
            Mode::Marks(MarkAction::New) => vec!["Save mark...".to_owned()],
            _ => {
                vec![
                    format!("{:?}", tab.mode.clone()),
                    format!("{}", tab.input.string.clone()),
                ]
            }
        };
        Ok(first_row)
    }

    fn tab_bar(&self, status: &Status) -> FmResult<usize> {
        let mut attr = Attr::default();
        self.term.print_with_attr(0, 0, "[", attr)?;
        let (number_of_tabs, selected_index) = status.len_index_of_tabs();

        for tab in 0..(number_of_tabs) {
            if tab == selected_index {
                attr = Attr {
                    bg: Color::default(),
                    fg: Color::CYAN,
                    effect: Effect::REVERSE,
                };
            }
            self.term
                .print_with_attr(0, 2 * tab + 1, &format!("{}", tab), attr)?;
            attr = Attr::default();
            self.term
                .print_with_attr(0, 2 * number_of_tabs, " ", Attr::default())?;
        }
        self.term
            .print_with_attr(0, 2 * number_of_tabs, "]", Attr::default())?;
        Ok(2 * number_of_tabs + 2)
    }

    fn draw_colored_strings(&self, offset: usize, first_row: Vec<String>) -> FmResult<()> {
        let mut col = 0;
        for (text, color) in std::iter::zip(first_row.iter(), Self::LINE_COLORS.iter().cycle()) {
            self.term.print_with_attr(0, offset + col, text, *color)?;
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
    fn files(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        let len = tab.path_content.files.len();
        for (i, (file, string)) in std::iter::zip(
            tab.path_content.files.iter(),
            tab.path_content.strings().iter(),
        )
        .enumerate()
        .take(min(len, tab.window.bottom + 1))
        .skip(tab.window.top)
        {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - tab.window.top;
            let mut attr = fileinfo_attr(status, file, &self.colors);
            if status.flagged.contains(&file.path) {
                attr.effect |= Effect::BOLD | Effect::UNDERLINE;
            }
            self.term.print_with_attr(row, 0, string, attr)?;
        }
        Ok(())
    }

    /// Display a cursor in the top row, at a correct column.
    fn cursor(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        match tab.mode {
            Mode::Normal | Mode::Help | Mode::Marks(_) => {
                self.term.show_cursor(false)?;
            }
            Mode::NeedConfirmation => {
                self.term.set_cursor(0, tab.last_edition.offset())?;
            }
            Mode::Sort => {
                self.term.set_cursor(0, Self::SORT_CURSOR_OFFSET)?;
            }
            _ => {
                self.term
                    .set_cursor(0, tab.input.cursor_index + Self::EDIT_BOX_OFFSET)?;
            }
        }
        Ok(())
    }

    /// Display the possible jump destination from flagged files.
    fn jump_list(&mut self, tabs: &Status) -> FmResult<()> {
        self.term.print(0, 0, "Jump to...")?;
        for (row, path) in tabs.flagged.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tabs.jump_index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::new("Unreadable filename"))?,
                attr,
            );
        }
        Ok(())
    }

    /// Display the history of visited directories.
    fn history(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        self.term.print(0, 0, "Go to...")?;
        for (row, path) in tab.history.visited.iter().rev().enumerate() {
            let mut attr = Attr::default();
            if row == tab.history.len() - tab.history.index - 1 {
                attr.effect |= Effect::REVERSE;
            }
            self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::new("Unreadable filename"))?,
                attr,
            )?;
        }
        Ok(())
    }

    /// Display the predefined shortcuts.
    fn shortcuts(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        self.term.print(0, 0, "Go to...")?;
        for (row, path) in tab.shortcut.shortcuts.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tab.shortcut.index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::new("Unreadable filename"))?,
                attr,
            );
        }
        Ok(())
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        self.term
            .set_cursor(0, tab.input.cursor_index + Self::EDIT_BOX_OFFSET)?;
        for (row, candidate) in tab.completion.proposals.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tab.completion.index {
                attr.effect |= Effect::REVERSE;
            }
            self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                candidate,
                attr,
            )?;
        }
        Ok(())
    }

    /// Display a list of edited (deleted, copied, moved) files for confirmation
    fn confirmation(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        for (row, path) in status.flagged.iter().enumerate() {
            self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                4,
                path.to_str()
                    .ok_or_else(|| FmError::new("Unreadable filename"))?,
                Attr::default(),
            )?;
        }
        info!("last_edition: {}", tab.last_edition);
        if let LastEdition::CopyPaste = tab.last_edition {
            let attr = Attr {
                fg: Color::YELLOW,
                bg: Color::Default,
                effect: Effect::BOLD,
            };
            let content = format!(
                "Files will be copied to {}",
                tab.path_content.path_to_str()?
            );
            self.term.print_with_attr(2, 3, &content, attr)?;
        }
        Ok(())
    }

    fn preview_line_numbers(
        &mut self,
        tab: &Tab,
        line_index: usize,
        row_index: usize,
    ) -> FmResult<usize> {
        Ok(self.term.print_with_attr(
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
    fn preview(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();

        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        match &tab.preview {
            // TODO: should it belong to separate methods ?
            Preview::Syntaxed(syntaxed) => {
                for (i, vec_line) in syntaxed
                    .content
                    .iter()
                    .enumerate()
                    .skip(tab.window.top)
                    .take(min(length, tab.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, tab);
                    self.preview_line_numbers(tab, i, row)?;
                    for token in vec_line.iter() {
                        token.print(&self.term, row, line_number_width)?;
                    }
                }
            }
            Preview::Text(text) => {
                for (i, line) in text
                    .content
                    .iter()
                    .enumerate()
                    .skip(tab.window.top)
                    .take(min(length, tab.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, tab);
                    self.term.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Binary(bin) => {
                let line_number_width_hex = format!("{:x}", bin.len() * 16).len();

                for (i, line) in bin
                    .content
                    .iter()
                    .enumerate()
                    .skip(tab.window.top)
                    .take(min(length, tab.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, tab);

                    self.term.print_with_attr(
                        row,
                        0,
                        &format_line_nr_hex(i + 1 + tab.window.top, line_number_width_hex),
                        Self::ATTR_LINE_NR,
                    )?;
                    line.print(&self.term, row, line_number_width_hex + 1);
                }
            }
            Preview::Pdf(text) => {
                for (i, line) in text
                    .content
                    .iter()
                    .enumerate()
                    .skip(tab.window.top)
                    .take(min(length, tab.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, tab);
                    self.term.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Compressed(zip) => {
                for (i, line) in zip
                    .content
                    .iter()
                    .enumerate()
                    .skip(tab.window.top)
                    .take(min(length, tab.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, tab);
                    self.term.print_with_attr(
                        row,
                        0,
                        &(i + 1 + tab.window.top).to_string(),
                        Self::ATTR_LINE_NR,
                    )?;
                    self.term.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Image(text) => {
                for (i, line) in text
                    .content
                    .iter()
                    .enumerate()
                    .skip(tab.window.top)
                    .take(min(length, tab.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, tab);
                    self.term.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Media(media) => {
                for (i, line) in media
                    .content
                    .iter()
                    .enumerate()
                    .skip(tab.window.top)
                    .take(min(length, tab.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, tab);
                    self.term.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Empty => (),
        }
        Ok(())
    }

    fn marks(&mut self, status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();

        self.term
            .print_with_attr(2, 1, "mark  path", Self::ATTR_YELLOW)?;

        for (i, line) in status.marks.as_strings()?.iter().enumerate() {
            let row = Self::calc_line_row(i, tab) + 2;
            self.term.print(row, 3, line)?;
        }
        Ok(())
    }

    fn calc_line_row(i: usize, status: &Tab) -> usize {
        i + ContentWindow::WINDOW_MARGIN_TOP - status.window.top
    }

    // fn disk_space(&mut self, path_str: String) -> String {
    //     self.sys.refresh_disks();
    //     let mut size = 0_u64;
    //     let mut disks: Vec<&sysinfo::Disk> = self.sys.disks().iter().collect();
    //     disks.sort_by_key(|disk| disk.mount_point().as_os_str().len());
    //     for disk in disks {
    //         if path_str.contains(disk.mount_point().as_os_str().to_str().unwrap()) {
    //             size = disk.available_space();
    //         };
    //     }
    //     human_size(size)
    // }
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
