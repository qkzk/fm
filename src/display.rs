use std::cmp::min;

use tuikit::attr::*;
use tuikit::term::Term;

use crate::config::Colors;
use crate::content_window::ContentWindow;
use crate::fileinfo::fileinfo_attr;
use crate::help::HELP_LINES;
use crate::last_edition::LastEdition;
use crate::mode::Mode;
use crate::preview::Preview;
use crate::status::Status;
use crate::tabs::Tabs;

/// Is responsible for displaying content in the terminal.
/// It uses an already created terminal.
pub struct Display {
    /// The Tuikit terminal attached to the display.
    /// It will print every symbol shown on screen.
    pub term: Term,
    colors: Colors,
}

impl Display {
    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Term, colors: Colors) -> Self {
        Self { term, colors }
    }

    const EDIT_BOX_OFFSET: usize = 10;
    const SORT_CURSOR_OFFSET: usize = 36;

    const LINE_ATTR: Attr = Attr {
        fg: Color::CYAN,
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
    pub fn display_all(&mut self, tabs: &Tabs) {
        let status = tabs.selected_non_mut();
        self.first_line(status, tabs);
        self.files(status, tabs);
        self.cursor(status);
        match status.mode {
            Mode::Help => self.help(),
            Mode::Jump => self.jump_list(tabs),
            Mode::History => self.history(status),
            Mode::Exec | Mode::Goto | Mode::Search => self.completion(status, tabs),
            Mode::NeedConfirmation => self.confirmation(status, tabs),
            Mode::Preview => self.preview(status, tabs),
            Mode::Shortcut => self.shortcuts(status),
            _ => (),
        }
    }

    /// Reads and returns the `tuikit::term::Term` height.
    pub fn height(&self) -> usize {
        let (_, height) = self.term.term_size().unwrap();
        height
    }

    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    fn first_line(&mut self, status: &Status, tabs: &Tabs) {
        let first_row: String = match status.mode {
            Mode::Normal => {
                format!(
                    "Tab: {}/{}  --  Path: {}   --   Files: {}",
                    tabs.index + 1,
                    tabs.len(),
                    status.path_content.path.to_str().unwrap(),
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
            _ => {
                format!("{:?} {}", status.mode.clone(), status.input.string.clone())
            }
        };
        let _ = self.term.print_with_attr(0, 0, &first_row, Self::LINE_ATTR);
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Those files are always shown, which make it a little bit faster in;
    /// normal (ie. default) mode.
    /// Where there's too much files, only those around the selected one are
    /// displayed.
    fn files(&mut self, status: &Status, tabs: &Tabs) {
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
            let _ = self.term.print_with_attr(row, 0, string, attr);
        }
    }

    /// Display a cursor in the top row, at a correct column.
    fn cursor(&mut self, status: &Status) {
        match status.mode {
            Mode::Normal | Mode::Help => {
                let _ = self.term.show_cursor(false);
            }
            Mode::NeedConfirmation => {
                let _ = self.term.set_cursor(0, status.last_edition.offset());
            }
            Mode::Sort => {
                let _ = self.term.set_cursor(0, Self::SORT_CURSOR_OFFSET);
            }
            _ => {
                let _ = self
                    .term
                    .set_cursor(0, status.input.cursor_index + Self::EDIT_BOX_OFFSET);
            }
        }
    }

    /// Display the help message with default keybindings.
    fn help(&mut self) {
        let _ = self.term.clear();
        for (row, line) in HELP_LINES.split('\n').enumerate() {
            let _ = self.term.print(row, 0, line);
        }
    }

    /// Display the possible jump destination from flagged files.
    fn jump_list(&mut self, tabs: &Tabs) {
        let _ = self.term.clear();
        let _ = self.term.print(0, 0, "Jump to...");
        for (row, path) in tabs.flagged.iter().enumerate() {
            let mut attr = Attr::default();
            if row == tabs.jump_index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().unwrap(),
                attr,
            );
        }
    }

    /// Display the history of visited directories.
    fn history(&mut self, status: &Status) {
        let _ = self.term.clear();
        let _ = self.term.print(0, 0, "Go to...");
        for (row, path) in status.history.visited.iter().rev().enumerate() {
            let mut attr = Attr::default();
            if row == status.history.len() - status.history.index - 1 {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().unwrap(),
                attr,
            );
        }
    }

    /// Display the predefined shortcuts.
    fn shortcuts(&mut self, status: &Status) {
        let _ = self.term.clear();
        let _ = self.term.print(0, 0, "Go to...");
        for (row, path) in status.shortcut.shortcuts.iter().enumerate() {
            let mut attr = Attr::default();
            if row == status.shortcut.index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().unwrap(),
                attr,
            );
        }
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&mut self, status: &Status, tabs: &Tabs) {
        let _ = self.term.clear();
        self.first_line(status, tabs);
        let _ = self
            .term
            .set_cursor(0, status.input.cursor_index + Self::EDIT_BOX_OFFSET);
        for (row, candidate) in status.completion.proposals.iter().enumerate() {
            let mut attr = Attr::default();
            if row == status.completion.index {
                attr.effect |= Effect::REVERSE;
            }
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                candidate,
                attr,
            );
        }
    }

    /// Display a list of edited (deleted, copied, moved) files for confirmation
    fn confirmation(&mut self, status: &Status, tabs: &Tabs) {
        let _ = self.term.clear();
        self.first_line(status, tabs);
        for (row, path) in tabs.flagged.iter().enumerate() {
            let _ = self.term.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP + 2,
                4,
                path.to_str().unwrap(),
                Attr::default(),
            );
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
                status.path_content.path.to_str().unwrap()
            );
            let _ = self.term.print_with_attr(2, 3, &content, attr);
        }
    }

    /// Display a scrollable preview of a file.
    /// Multiple modes are supported :
    /// if the filename extension is recognized, the preview is highlighted,
    /// if the file content is recognized as binary, an hex dump is previewed with 16 bytes lines,
    /// else the content is supposed to be text and shown as such.
    /// It may fail to recognize some usual extensions, notably `.toml`.
    /// It may fail to recognize small files (< 1024 bytes).
    fn preview(&mut self, status: &Status, tabs: &Tabs) {
        if let Mode::Preview = status.mode {
            let _ = self.term.clear();
            self.first_line(status, tabs);

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
                        let _ = self.term.print_with_attr(
                            row,
                            0,
                            &(i + 1 + status.window.top).to_string(),
                            Self::LINE_ATTR,
                        );
                        for token in vec_line.iter() {
                            token.print(&self.term, row, line_number_width);
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
                        let _ = self.term.print_with_attr(
                            row,
                            0,
                            &(i + 1 + status.window.top).to_string(),
                            Self::LINE_ATTR,
                        );
                        let _ = self.term.print(row, line_number_width + 3, line);
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

                        let _ = self.term.print_with_attr(
                            row,
                            0,
                            &format_line_nr_hex(i + 1 + status.window.top, line_number_width_hex),
                            Self::LINE_ATTR,
                        );
                        line.print(&self.term, row, line_number_width_hex + 1);
                    }
                }
                Preview::Empty => (),
            }
        }
    }

    fn calc_line_row(i: usize, status: &Status) -> usize {
        i + ContentWindow::WINDOW_MARGIN_TOP - status.window.top
    }
}

fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
    format!("{:0width$x}", line_nr)
}
