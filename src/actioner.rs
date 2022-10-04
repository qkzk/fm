use std::collections::HashMap;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::config::Keybindings;
use crate::event_char::EventChar;
use crate::mode::Mode;
use crate::status::Status;

/// Struct which mutates `Status`.
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on status.
/// Keybindings are read from `Config`.
pub struct Actioner {
    binds: HashMap<char, EventChar>,
}

impl Actioner {
    /// Creates a map of configurable keybindings to `EventChar`
    /// The `EventChar` is then associated to a `Status` method.
    pub fn new(keybindings: &Keybindings) -> Self {
        let binds = HashMap::from([
            (keybindings.toggle_hidden, EventChar::ToggleHidden),
            (keybindings.copy_paste, EventChar::CopyPaste),
            (keybindings.cut_paste, EventChar::CutPaste),
            (keybindings.newdir, EventChar::NewDir),
            (keybindings.newfile, EventChar::NewFile),
            (keybindings.chmod, EventChar::Chmod),
            (keybindings.exec, EventChar::Exec),
            (keybindings.goto, EventChar::Goto),
            (keybindings.rename, EventChar::Rename),
            (keybindings.clear_flags, EventChar::ClearFlags),
            (keybindings.toggle_flag, EventChar::ToggleFlag),
            (keybindings.shell, EventChar::Shell),
            (keybindings.delete, EventChar::DeleteFile),
            (keybindings.open_file, EventChar::OpenFile),
            (keybindings.help, EventChar::Help),
            (keybindings.search, EventChar::Search),
            (keybindings.regex_match, EventChar::RegexMatch),
            (keybindings.quit, EventChar::Quit),
            (keybindings.flag_all, EventChar::FlagAll),
            (keybindings.reverse_flags, EventChar::ReverseFlags),
            (keybindings.jump, EventChar::Jump),
            (keybindings.nvim, EventChar::NvimFilepicker),
            (keybindings.sort_by, EventChar::Sort),
            (keybindings.symlink, EventChar::Symlink),
            (keybindings.preview, EventChar::Preview),
        ]);
        Self { binds }
    }
    /// Reaction to received events.
    pub fn read_event(&self, status: &mut Status, ev: Event) {
        match ev {
            Event::Key(Key::ESC) => self.escape(status),
            Event::Key(Key::Up) => self.up(status),
            Event::Key(Key::Down) => self.down(status),
            Event::Key(Key::Left) => self.left(status),
            Event::Key(Key::Right) => self.right(status),
            Event::Key(Key::Backspace) => self.backspace(status),
            Event::Key(Key::Ctrl('d')) => self.delete(status),
            Event::Key(Key::Delete) => self.delete(status),
            Event::Key(Key::Char(c)) => self.char(status, c),
            Event::Key(Key::Home) => self.home(status),
            Event::Key(Key::End) => self.end(status),
            Event::Key(Key::PageDown) => self.page_down(status),
            Event::Key(Key::PageUp) => self.page_up(status),
            Event::Key(Key::Enter) => self.enter(status),
            Event::Key(Key::Tab) => self.tab(status),
            Event::Key(Key::WheelUp(_, _, _)) => self.up(status),
            Event::Key(Key::WheelDown(_, _, _)) => self.down(status),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => self.left_click(status, row),
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                self.right_click(status, row)
            }
            _ => {}
        }
    }

    /// Leaving a mode reset the window
    fn escape(&self, status: &mut Status) {
        status.event_normal()
    }

    /// Move one line up
    fn up(&self, status: &mut Status) {
        match status.mode {
            Mode::Normal => status.event_up_one_row(),
            Mode::Jump => status.event_jumplist_prev(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.completion.prev();
            }
            _ => (),
        }
    }

    /// Move one line down
    fn down(&self, status: &mut Status) {
        match status.mode {
            Mode::Normal => status.event_down_one_row(),
            Mode::Jump => status.event_jumplist_next(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.completion.next();
            }
            _ => (),
        }
    }

    /// Move left in a string, move to parent in normal mode
    fn left(&self, status: &mut Status) {
        match status.mode {
            Mode::Normal => status.event_move_to_parent(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => status.event_move_cursor_left(),
            _ => (),
        }
    }

    /// Move right in a string, move to children in normal mode.
    fn right(&self, status: &mut Status) {
        match status.mode {
            Mode::Normal => status.event_go_to_child(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => status.event_move_cursor_right(),
            _ => (),
        }
    }

    /// Deletes a char in input string
    fn backspace(&self, status: &mut Status) {
        match status.mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => status.event_delete_char_left(),
            Mode::Normal => (),
            _ => (),
        }
    }

    /// Deletes chars right of cursor in input string.
    fn delete(&self, status: &mut Status) {
        match status.mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => status.event_delete_chars_right(),
            Mode::Normal => (),
            _ => (),
        }
    }

    /// Move to top or beggining of line.
    fn home(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            status.event_go_top()
        } else {
            status.event_cursor_home()
        }
    }

    /// Move to end or end of line.
    fn end(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            status.event_go_bottom()
        } else {
            status.event_cursor_end()
        }
    }

    /// Move down 10 rows
    fn page_down(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            status.event_down_10_rows()
        }
    }

    /// Move up 10 rows
    fn page_up(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            status.event_up_10_rows()
        }
    }

    /// Execute a command
    fn enter(&self, status: &mut Status) {
        match status.mode {
            Mode::Rename => status.exec_rename(),
            Mode::Newfile => status.exec_newfile(),
            Mode::Newdir => status.exec_newdir(),
            Mode::Chmod => status.exec_chmod(),
            Mode::Exec => status.exec_exec(),
            Mode::Search => status.exec_search(),
            Mode::Goto => status.exec_goto(),
            Mode::RegexMatch => status.exec_regex(),
            Mode::Jump => status.exec_jump(),
            Mode::Normal | Mode::NeedConfirmation | Mode::Help | Mode::Sort | Mode::Preview => (),
        }

        status.input.reset();
        status.mode = Mode::Normal;
    }

    /// Select this file
    fn left_click(&self, status: &mut Status, row: u16) {
        if let Mode::Normal = status.mode {
            status.event_select_row(row)
        }
    }

    /// Open a directory or a file
    fn right_click(&self, status: &mut Status, row: u16) {
        if let Mode::Normal = status.mode {
            status.event_right_click(row)
        }
    }

    /// Select next completion and insert it
    fn tab(&self, status: &mut Status) {
        match status.mode {
            Mode::Goto | Mode::Exec | Mode::Search => status.event_replace_input_with_completion(),
            _ => (),
        }
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(&self, status: &mut Status, c: char) {
        match status.mode {
            Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename | Mode::RegexMatch => {
                status.event_text_insertion(c)
            }
            Mode::Goto | Mode::Exec | Mode::Search => status.event_text_insert_and_complete(c),
            Mode::Normal => match self.binds.get(&c) {
                Some(event_char) => event_char.match_char(status),
                None => (),
            },
            Mode::Help | Mode::Preview => status.event_normal(),
            Mode::Jump => (),
            Mode::NeedConfirmation => {
                if c == 'y' {
                    status.exec_last_edition()
                }
                status.event_leave_need_confirmation()
            }
            Mode::Sort => status.event_leave_sort(c),
        }
    }
}
