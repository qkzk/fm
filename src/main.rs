extern crate shellexpand;

use std::cmp::min;

use clap::Parser;
use tuikit::attr::*;
use tuikit::event::{Event, Key};
use tuikit::key::MouseButton;
use tuikit::term::{Term, TermHeight};

use fm::args::Args;
use fm::config::load_config;
use fm::file_window::WINDOW_MARGIN_TOP;
use fm::fileinfo::fileinfo_attr;
use fm::help::HELP_LINES;
use fm::mode::Mode;
use fm::status::Status;

const EDIT_BOX_OFFSET: usize = 10;
const SORT_CURSOR_OFFSET: usize = 29;

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

struct Display {
    term: Term,
}

impl Display {
    fn new(term: Term) -> Self {
        Self { term }
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

    fn completion(&mut self, status: &Status) {
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

fn main() {
    let args = Args::parse();
    eprintln!("Clap args {:?}", args);
    let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    let _ = term.enable_mouse_support();
    let (_, height) = term.term_size().unwrap();
    let config = load_config(CONFIG_PATH);
    let mut status = Status::new(args, config, height);
    let mut display = Display::new(term);
    while let Ok(ev) = display.term.poll_event() {
        let _ = display.term.clear();
        let (_width, height) = display.term.term_size().unwrap();
        status.set_height(height);
        eprintln!("{:?}", ev);
        match ev {
            Event::Key(Key::ESC) => status.event_esc(),
            Event::Key(Key::Up) => status.event_up(),
            Event::Key(Key::Down) => status.event_down(),
            Event::Key(Key::Left) => status.event_left(),
            Event::Key(Key::Right) => status.event_right(),
            Event::Key(Key::Backspace) => status.event_backspace(),
            Event::Key(Key::Ctrl('d')) => status.event_delete_char(),
            Event::Key(Key::Delete) => status.event_delete_char(),
            Event::Key(Key::Char(c)) => status.event_char(c),
            Event::Key(Key::Home) => status.event_home(),
            Event::Key(Key::End) => status.event_end(),
            Event::Key(Key::PageDown) => status.event_page_down(),
            Event::Key(Key::PageUp) => status.event_page_up(),
            Event::Key(Key::Enter) => status.event_enter(),
            Event::Key(Key::Tab) => status.event_complete(),
            Event::Key(Key::WheelUp(_, _, _)) => status.event_up(),
            Event::Key(Key::WheelDown(_, _, _)) => status.event_down(),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => status.event_left_click(row),
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                status.event_right_click(row)
            }
            _ => {}
        }

        display.first_line(&status);
        display.files(&status);
        display.help_or_cursor(&status);
        display.jump_list(&status);
        display.completion(&status);

        let _ = display.term.present();
    }
}
