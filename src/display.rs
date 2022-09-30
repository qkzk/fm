use std::cmp::min;

use tuikit::attr::*;
use tuikit::term::Term;

use crate::file_window::WINDOW_MARGIN_TOP;
use crate::fileinfo::fileinfo_attr;
use crate::help::HELP_LINES;
use crate::mode::Mode;
use crate::status::Status;

const EDIT_BOX_OFFSET: usize = 10;
const SORT_CURSOR_OFFSET: usize = 29;

pub struct Display {
    pub term: Term,
}

impl Display {
    pub fn new(term: Term) -> Self {
        Self { term }
    }

    pub fn display_all(&mut self, status: &Status) {
        self.first_line(status);
        self.files(status);
        self.help_or_cursor(status);
        self.jump_list(status);
        self.completion(status);
    }

    pub fn height(&self) -> usize {
        let (_, height) = self.term.term_size().unwrap();
        height
    }

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
            _ => {
                format!("{:?} {}", status.mode.clone(), status.input_string.clone())
            }
        };
        let _ = self.term.print(0, 0, &first_row);
    }

    fn files(&mut self, status: &Status) {
        let strings = status.path_content.strings();
        for (i, string) in strings
            .iter()
            .enumerate()
            .take(min(strings.len(), status.window.bottom + 1))
            .skip(status.window.top)
        {
            let row = i + WINDOW_MARGIN_TOP - status.window.top;
            let mut attr = fileinfo_attr(&status.path_content.files[i], &status.config.colors);
            if status.flagged.contains(&status.path_content.files[i].path) {
                attr.effect |= Effect::UNDERLINE;
            }
            let _ = self.term.print_with_attr(row, 0, string, attr);
        }
    }

    fn help_or_cursor(&mut self, status: &Status) {
        match status.mode {
            Mode::Normal => {
                let _ = self.term.set_cursor(0, 0);
            }
            Mode::Help => {
                let _ = self.term.clear();
                for (row, line) in HELP_LINES.split('\n').enumerate() {
                    let _ = self.term.print(row, 0, line);
                }
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
                    .set_cursor(0, status.input_string_cursor_index + EDIT_BOX_OFFSET);
            }
        }
    }

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

    pub fn completion(&mut self, status: &Status) {
        match status.mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                let _ = self.term.clear();
                self.first_line(status);
                let _ = self
                    .term
                    .set_cursor(0, status.input_string_cursor_index + EDIT_BOX_OFFSET);
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
}
