use std::cmp::min;
use std::sync::Arc;

use tuikit::attr::*;
use tuikit::term::Term;

use crate::config::Colors;
use crate::content_window::ContentWindow;
use crate::fileinfo::fileinfo_attr;
use crate::fm_error::FmResult;
use crate::last_edition::LastEdition;
use crate::mode::{MarkAction, Mode};
use crate::preview::Preview;
use crate::status::Status;
use crate::tab::Tab;

/// Is responsible for displaying content in the terminal.
/// It uses an already created terminal.
pub struct Display {
    /// The Tuikit terminal attached to the display.
    /// It will print every symbol shown on screen.
    pub term: Arc<Term>,
    colors: Colors,
}

impl Display {
    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Arc<Term>, colors: Colors) -> Self {
        Self { term, colors }
    }

    const EDIT_BOX_OFFSET: usize = 10;
    const SORT_CURSOR_OFFSET: usize = 36;
    const ATTR_LINE_NR: Attr = Attr {
        fg: Color::CYAN,
        bg: Color::Default,
        effect: Effect::empty(),
    };
    const ATTR_YELLOW: Attr = Attr {
        fg: Color::YELLOW,
        bg: Color::Default,
        effect: Effect::empty(),
    };
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
    pub fn display_all(&mut self, tabs: &Status) -> FmResult<()> {
        let status = tabs.selected_non_mut();
        self.first_line(status, tabs)?;
        self.files(status, tabs)?;
        self.cursor(status)?;
        match status.mode {
            // Mode::Help => self.help(),
            Mode::Jump => self.jump_list(tabs),
            Mode::History => self.history(status),
            Mode::Exec | Mode::Goto | Mode::Search => self.completion(status, tabs),
            Mode::NeedConfirmation => self.confirmation(status, tabs),
            Mode::Preview | Mode::Help => self.preview(status, tabs),
            Mode::Shortcut => self.shortcuts(status),
            Mode::Marks(MarkAction::New) | Mode::Marks(MarkAction::Jump) => {
                self.marks(status, tabs)
            }
            _ => Ok(()),
        }
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
    fn first_line(&mut self, status: &Tab, tabs: &Status) -> FmResult<()> {
        let first_row: String = match status.mode {
            Mode::Normal => {
                format!(
                    "Tab: {}/{}  --  Path: {}   --   Files: {}",
                    tabs.index + 1,
                    tabs.len(),
                    status.path_content.path_to_str(),
                    status.path_content.files.len(),
                )
            }
            Mode::NeedConfirmation => {
                format!("Confirm {} (y/n) : ", status.last_edition)
            }
            Mode::Preview => match status.path_content.selected_file() {
                Some(fileinfo) => format!(
                    "{:?} {}",
                    status.mode.clone(),
                    fileinfo.path.to_string_lossy()
                ),
                None => "".to_owned(),
            },
            Mode::Help => "fm: a dired like file manager. Default keybindings.".to_owned(),
            Mode::Marks(MarkAction::Jump) => "Jump to...".to_owned(),
            Mode::Marks(MarkAction::New) => "Save mark...".to_owned(),
            _ => {
                format!("{:?} {}", status.mode.clone(), status.input.string.clone())
            }
        };
        self.term
            .print_with_attr(0, 0, &first_row, Self::ATTR_LINE_NR)?;
        Ok(())
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Those files are always shown, which make it a little bit faster in;
    /// normal (ie. default) mode.
    /// Where there's too much files, only those around the selected one are
    /// displayed.
    fn files(&mut self, status: &Tab, tabs: &Status) -> FmResult<()> {
        let strings = status.path_content.strings();
        for (i, string) in strings
            .iter()
            .enumerate()
            .take(min(strings.len(), status.window.bottom + 1))
            .skip(status.window.top)
        {
            let row = i + ContentWindow::WINDOW_MARGIN_TOP - status.window.top;
            let mut attr = fileinfo_attr(&status.path_content.files[i], &self.colors);
            if tabs.flagged.contains(&status.path_content.files[i].path) {
                attr.effect |= Effect::UNDERLINE;
            }
            self.term.print_with_attr(row, 0, string, attr)?;
        }
        Ok(())
    }

    /// Display a cursor in the top row, at a correct column.
    fn cursor(&mut self, status: &Tab) -> FmResult<()> {
        match status.mode {
            Mode::Normal | Mode::Help | Mode::Marks(_) => {
                self.term.show_cursor(false)?;
            }
            Mode::NeedConfirmation => {
                self.term.set_cursor(0, status.last_edition.offset())?;
            }
            Mode::Sort => {
                self.term.set_cursor(0, Self::SORT_CURSOR_OFFSET)?;
            }
            _ => {
                self.term
                    .set_cursor(0, status.input.cursor_index + Self::EDIT_BOX_OFFSET)?;
            }
        }
        Ok(())
    }

    /// Display the possible jump destination from flagged files.
    fn jump_list(&mut self, tabs: &Status) -> FmResult<()> {
        self.term.clear()?;
        self.term.print(0, 0, "Jump to...")?;
        for (row, path) in tabs.flagged.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tabs.jump_index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().unwrap_or_default(),
                attr,
            );
        }
        Ok(())
    }

    /// Display the history of visited directories.
    fn history(&mut self, status: &Tab) -> FmResult<()> {
        self.term.clear()?;
        self.term.print(0, 0, "Go to...")?;
        for (row, path) in status.history.visited.iter().rev().enumerate() {
            let mut attr = Attr::default();
            if row == status.history.len() - status.history.index - 1 {
                attr.effect |= Effect::REVERSE;
            }
            self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().unwrap_or_default(),
                attr,
            )?;
        }
        Ok(())
    }

    /// Display the predefined shortcuts.
    fn shortcuts(&mut self, status: &Tab) -> FmResult<()> {
        self.term.clear()?;
        self.term.print(0, 0, "Go to...")?;
        for (row, path) in status.shortcut.shortcuts.iter().enumerate() {
            let mut attr = Attr::default();
            if row == status.shortcut.index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().unwrap_or_default(),
                attr,
            );
        }
        Ok(())
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&mut self, status: &Tab, tabs: &Status) -> FmResult<()> {
        self.term.clear()?;
        self.first_line(status, tabs)?;
        self.term
            .set_cursor(0, status.input.cursor_index + Self::EDIT_BOX_OFFSET)?;
        for (row, candidate) in status.completion.proposals.iter().enumerate() {
            let mut attr = Attr::default();
            if row == status.completion.index {
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
    fn confirmation(&mut self, status: &Tab, tabs: &Status) -> FmResult<()> {
        self.term.clear()?;
        self.first_line(status, tabs)?;
        for (row, path) in tabs.flagged.iter().enumerate() {
            self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                4,
                path.to_str().unwrap_or_default(),
                Attr::default(),
            )?;
        }
        eprintln!("last_edition: {}", status.last_edition);
        if let LastEdition::CopyPaste = status.last_edition {
            let attr = Attr {
                fg: Color::YELLOW,
                bg: Color::Default,
                effect: Effect::BOLD,
            };
            let content = format!(
                "Files will be copied to {}",
                status.path_content.path_to_str()
            );
            self.term.print_with_attr(2, 3, &content, attr)?;
        }
        Ok(())
    }

    /// Display a scrollable preview of a file.
    /// Multiple modes are supported :
    /// if the filename extension is recognized, the preview is highlighted,
    /// if the file content is recognized as binary, an hex dump is previewed with 16 bytes lines,
    /// else the content is supposed to be text and shown as such.
    /// It may fail to recognize some usual extensions, notably `.toml`.
    /// It may fail to recognize small files (< 1024 bytes).
    fn preview(&mut self, status: &Tab, tabs: &Status) -> FmResult<()> {
        self.term.clear()?;
        self.first_line(status, tabs)?;

        let length = status.preview.len();
        let line_number_width = length.to_string().len();
        match &status.preview {
            // TODO: should it belong to separate methods ?
            Preview::Syntaxed(syntaxed) => {
                for (i, vec_line) in syntaxed
                    .content
                    .iter()
                    .enumerate()
                    .skip(status.window.top)
                    .take(min(length, status.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, status);
                    self.term.print_with_attr(
                        row,
                        0,
                        &(i + 1 + status.window.top).to_string(),
                        Self::ATTR_LINE_NR,
                    )?;
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
                    .skip(status.window.top)
                    .take(min(length, status.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, status);
                    self.term.print_with_attr(
                        row,
                        0,
                        &(i + 1 + status.window.top).to_string(),
                        Self::ATTR_LINE_NR,
                    )?;
                    self.term.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Binary(bin) => {
                let line_number_width_hex = format!("{:x}", bin.len() * 16).len();

                for (i, line) in bin
                    .content
                    .iter()
                    .enumerate()
                    .skip(status.window.top)
                    .take(min(length, status.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, status);

                    self.term.print_with_attr(
                        row,
                        0,
                        &format_line_nr_hex(i + 1 + status.window.top, line_number_width_hex),
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
                    .skip(status.window.top)
                    .take(min(length, status.window.bottom + 1))
                {
                    let row = Self::calc_line_row(i, status);
                    self.term.print_with_attr(
                        row,
                        0,
                        &(i + 1 + status.window.top).to_string(),
                        Self::ATTR_LINE_NR,
                    )?;
                    self.term.print(row, line_number_width + 3, line)?;
                }
            }
            Preview::Empty => (),
        }
        Ok(())
    }

    fn marks(&mut self, status: &Tab, tabs: &Status) -> FmResult<()> {
        self.term.clear()?;
        self.first_line(status, tabs)?;

        self.term
            .print_with_attr(2, 1, "mark  path", Self::ATTR_YELLOW)?;

        for (i, line) in tabs.marks.as_strings().iter().enumerate() {
            let row = Self::calc_line_row(i, status) + 2;
            self.term.print(row, 3, line)?;
        }
        Ok(())
    }

    fn calc_line_row(i: usize, status: &Tab) -> usize {
        i + ContentWindow::WINDOW_MARGIN_TOP - status.window.top
    }
}

fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
    format!("{:0width$x}", line_nr)
}
