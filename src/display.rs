use std::cmp::min;

use tuikit::attr::*;
use tuikit::term::Term;

use crate::config::Colors;
use crate::file_window::WINDOW_MARGIN_TOP;
use crate::fileinfo::fileinfo_attr;
use crate::help::HELP_LINES;
use crate::mode::Mode;
use crate::status::Status;

const EDIT_BOX_OFFSET: usize = 10;
const SORT_CURSOR_OFFSET: usize = 36;
static LINE_ATTR: Attr = Attr {
    fg: Color::CYAN,
    bg: Color::Default,
    effect: Effect::empty(),
};

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
    pub fn display_all(&mut self, status: &Status) {
        self.first_line(status);
        self.files(status);
        self.cursor(status);
        self.help(status);
        self.jump_list(status);
        self.completion(status);
        self.preview(status);
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
    fn first_line(&mut self, status: &Status) {
        let first_row: String = match status.mode {
            Mode::Normal => {
                format!(
                    "Path: {}   --   {} files",
                    status.path_content.path.to_str().unwrap(),
                    status.path_content.files.len(),
                )
            }
            Mode::NeedConfirmation => {
                format!("Confirm {} (y/n) : ", status.last_edition)
            }
            Mode::Preview => match status.path_content.selected_file() {
                Some(fileinfo) => format!("{:?} {}", status.mode.clone(), fileinfo.filename),
                None => "".to_owned(),
            },
            _ => {
                format!("{:?} {}", status.mode.clone(), status.input.string.clone())
            }
        };
        let _ = self.term.print_with_attr(0, 0, &first_row, LINE_ATTR);
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Those files are always shown, which make it a little bit faster in;
    /// normal (ie. default) mode.
    /// Where there's too much files, only those around the selected one are
    /// displayed.
    fn files(&mut self, status: &Status) {
        let strings = status.path_content.strings();
        for (i, string) in strings
            .iter()
            .enumerate()
            .take(min(strings.len(), status.window.bottom + 1))
            .skip(status.window.top)
        {
            let row = i + WINDOW_MARGIN_TOP - status.window.top;
            let mut attr = fileinfo_attr(&status.path_content.files[i], &self.colors);
            if status.flagged.contains(&status.path_content.files[i].path) {
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
                let _ = self.term.set_cursor(0, SORT_CURSOR_OFFSET);
            }
            _ => {
                let _ = self
                    .term
                    .set_cursor(0, status.input.cursor_index + EDIT_BOX_OFFSET);
            }
        }
    }

    /// Display the help message with default keybindings.
    fn help(&mut self, status: &Status) {
        if let Mode::Help = status.mode {
            let _ = self.term.clear();
            for (row, line) in HELP_LINES.split('\n').enumerate() {
                let _ = self.term.print(row, 0, line);
            }
        };
    }

    /// Display the possible jump destination from flagged files.
    fn jump_list(&mut self, status: &Status) {
        if let Mode::Jump = status.mode {
            let _ = self.term.clear();
            let _ = self.term.print(0, 0, "Jump to...");
            for (row, path) in status.flagged.iter().enumerate() {
                let mut attr = Attr::default();
                if row == status.jump_index {
                    attr.effect |= Effect::REVERSE;
                }
                let _ = self
                    .term
                    .print_with_attr(row + 1, 4, path.to_str().unwrap(), attr);
            }
        }
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&mut self, status: &Status) {
        match status.mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                let _ = self.term.clear();
                self.first_line(status);
                let _ = self
                    .term
                    .set_cursor(0, status.input.cursor_index + EDIT_BOX_OFFSET);
                for (row, candidate) in status.completion.proposals.iter().enumerate() {
                    let mut attr = Attr::default();
                    if row == status.completion.index {
                        attr.effect |= Effect::REVERSE;
                    }
                    let _ = self.term.print_with_attr(row + 1, 4, candidate, attr);
                }
            }
            _ => (),
        }
    }

    /// Display a scrollable preview of a file.
    fn preview(&mut self, status: &Status) {
        if let Mode::Preview = status.mode {
            let _ = self.term.clear();
            self.first_line(status);
            let content_attr = Attr::default();
            let line_number_offset = status.preview_lines.len().to_string().len() + 2;
            for (i, line) in status
                .preview_lines
                .iter()
                .enumerate()
                .take(min(status.preview_lines.len(), status.window.bottom + 1))
                .skip(status.window.top)
            {
                let row = i + WINDOW_MARGIN_TOP - status.window.top;
                let _ = self
                    .term
                    .print_with_attr(row + 2, 0, &(i + 1).to_string(), LINE_ATTR);
                let _ = self
                    .term
                    .print_with_attr(row + 2, line_number_offset, line, content_attr);
            }
        }
    }
}
