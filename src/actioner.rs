use std::collections::HashMap;
use std::sync::Arc;
use tuikit::prelude::{Event, Key, MouseButton};
use tuikit::term::Term;

use crate::config::Keybindings;
use crate::event_char::EventChar;
use crate::fm_error::{FmError, FmResult};
use crate::mode::{MarkAction, Mode};
use crate::skim::Skimer;
use crate::status::Status;

/// Struct which mutates `tabs.selected()..
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on tabs.selected().
/// Keybindings are read from `Config`.
pub struct Actioner {
    binds: HashMap<char, EventChar>,
    term: Arc<Term>,
}

impl Actioner {
    /// Creates a map of configurable keybindings to `EventChar`
    /// The `EventChar` is then associated to a `tabs.selected(). method.
    pub fn new(keybindings: &Keybindings, term: Arc<Term>) -> Self {
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
            (keybindings.history, EventChar::History),
            (keybindings.shortcut, EventChar::Shortcut),
            (keybindings.bulkrename, EventChar::Bulkrename),
            (keybindings.marks_new, EventChar::MarksNew),
            (keybindings.marks_jump, EventChar::MarksJump),
            (keybindings.filter, EventChar::Filter),
        ]);
        Self { binds, term }
    }
    /// Reaction to received events.
    pub fn read_event(&self, status: &mut Status, ev: Event) -> FmResult<()> {
        match ev {
            Event::Key(Key::ESC) => self.escape(status),
            Event::Key(Key::Up) => self.up(status),
            Event::Key(Key::Down) => self.down(status),
            Event::Key(Key::Left) => self.left(status),
            Event::Key(Key::Right) => self.right(status),
            Event::Key(Key::Backspace) => self.backspace(status),
            Event::Key(Key::Ctrl('d')) => self.delete(status),
            Event::Key(Key::Ctrl('q')) => self.escape(status),
            Event::Key(Key::Delete) => self.delete(status),
            Event::Key(Key::Insert) => self.insert(status),
            Event::Key(Key::Char(c)) => self.char(status, c),
            Event::Key(Key::Home) => self.home(status),
            Event::Key(Key::End) => self.end(status),
            Event::Key(Key::PageDown) => self.page_down(status),
            Event::Key(Key::PageUp) => self.page_up(status),
            Event::Key(Key::Enter) => self.enter(status),
            Event::Key(Key::Tab) => self.tab(status),
            Event::Key(Key::WheelUp(_, _, _)) => self.up(status),
            Event::Key(Key::WheelDown(_, _, _)) => self.down(status),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => {
                self.left_click(status, row);
                Ok(())
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                self.right_click(status, row);
                Ok(())
            }
            Event::Key(Key::Ctrl('f')) => self.ctrl_f(status),
            Event::Key(Key::Ctrl('c')) => self.ctrl_c(status),
            Event::Key(Key::Ctrl('p')) => self.ctrl_p(status),
            Event::User(_) => {
                eprintln!("read user event from user");
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Leaving a mode reset the window
    fn escape(&self, status: &mut Status) -> FmResult<()> {
        status.selected().event_normal()
    }

    /// Move one line up
    fn up(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => status.selected().event_up_one_row(),
            Mode::Jump => status.event_jumplist_prev(),
            Mode::History => status.selected().event_history_prev(),
            Mode::Shortcut => status.selected().event_shortcut_prev(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().completion.prev();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move one line down
    fn down(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => status.selected().event_down_one_row(),
            Mode::Jump => status.event_jumplist_next(),
            Mode::History => status.selected().event_history_next(),
            Mode::Shortcut => status.selected().event_shortcut_next(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().completion.next();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move left in a string, move to parent in normal mode
    fn left(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => status.selected().event_move_to_parent(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::Filter => {
                status.selected().event_move_cursor_left();
                Ok(())
            }

            _ => Ok(()),
        }
    }

    /// Move right in a string, move to children in normal mode.
    fn right(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => status.selected().event_child_or_open(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::Filter => {
                status.selected().event_move_cursor_right();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Deletes a char in input string
    fn backspace(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::Filter => {
                status.selected().event_delete_char_left();
                Ok(())
            }
            Mode::Normal => Ok(()),
            _ => Ok(()),
        }
    }

    /// Deletes chars right of cursor in input string.
    /// Remove current tab in normal mode.
    fn delete(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::Filter => {
                status.selected().event_delete_chars_right();
                Ok(())
            }

            Mode::Normal => {
                status.drop_tab();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Insert a new tab in normal mode
    fn insert(&self, status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected().mode {
            status.new_tab()
        };
        Ok(())
    }

    /// Move to top or beggining of line.
    fn home(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => status.selected().event_go_top(),
            _ => status.selected().event_cursor_home(),
        };
        Ok(())
    }

    /// Move to end or end of line.
    fn end(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => status.selected().event_go_bottom(),
            _ => status.selected().event_cursor_end(),
        };
        Ok(())
    }

    /// Move down 10 rows
    fn page_down(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => status.selected().event_page_down(),
            _ => (),
        };
        Ok(())
    }

    /// Move up 10 rows
    fn page_up(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => status.selected().event_page_up(),
            _ => (),
        };
        Ok(())
    }

    /// Execute a command
    fn enter(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Rename => status.selected().exec_rename()?,
            Mode::Newfile => status.selected().exec_newfile()?,
            Mode::Newdir => status.selected().exec_newdir()?,
            Mode::Chmod => status.exec_chmod()?,
            Mode::Exec => status.selected().exec_exec()?,
            Mode::Search => status.selected().exec_search(),
            Mode::Goto => status.selected().exec_goto()?,
            Mode::RegexMatch => status.exec_regex()?,
            Mode::Jump => status.exec_jump()?,
            Mode::History => status.selected().exec_history()?,
            Mode::Shortcut => status.selected().exec_shortcut()?,
            Mode::Filter => status.selected().exec_filter()?,
            Mode::Normal
            | Mode::NeedConfirmation
            | Mode::Help
            | Mode::Sort
            | Mode::Preview
            | Mode::Marks(_) => (),
        }

        status.selected().input.reset();
        status.selected().mode = Mode::Normal;
        Ok(())
    }

    /// Select this file
    fn left_click(&self, status: &mut Status, row: u16) {
        if let Mode::Normal = status.selected().mode {
            status.selected().event_select_row(row)
        }
    }

    /// Open a directory or a file
    fn right_click(&self, status: &mut Status, row: u16) {
        if let Mode::Normal = status.selected().mode {
            let _ = status.selected().event_right_click(row);
        }
    }

    /// Select next completion and insert it
    fn tab(&self, status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().event_replace_input_with_completion()
            }
            Mode::Normal => status.next(),
            _ => (),
        };
        Ok(())
    }

    fn ctrl_f(&self, status: &mut Status) -> FmResult<()> {
        let output = Skimer::new(self.term.clone()).no_source(
            status
                .selected_non_mut()
                .path_str()
                .ok_or_else(|| FmError::new("skim error"))?,
        );
        let _ = self.term.clear();
        status.create_tabs_from_skim(output);
        Ok(())
    }

    fn ctrl_c(&self, status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return status.selected_non_mut().event_filename_to_clipboard();
        }
        Ok(())
    }

    fn ctrl_p(&self, status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return status.selected_non_mut().event_filepath_to_clipboard();
        }
        Ok(())
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(&self, status: &mut Status, c: char) -> FmResult<()> {
        match status.selected().mode {
            Mode::Newfile
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Rename
            | Mode::RegexMatch
            | Mode::Filter => {
                status.selected().event_text_insertion(c);
                Ok(())
            }
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().event_text_insert_and_complete(c)
            }
            Mode::Normal => match self.binds.get(&c) {
                Some(event_char) => event_char.match_char(status),
                None => Ok(()),
            },
            Mode::Help | Mode::Preview | Mode::Shortcut => status.selected().event_normal(),
            Mode::Jump => Ok(()),
            Mode::History => Ok(()),
            Mode::NeedConfirmation => {
                if c == 'y' {
                    let _ = status.exec_last_edition();
                }
                status.selected().event_leave_need_confirmation();
                Ok(())
            }
            Mode::Marks(MarkAction::Jump) => status.exec_marks_jump(c),
            Mode::Marks(MarkAction::New) => status.exec_marks_new(c),
            Mode::Sort => {
                status.selected().event_leave_sort(c);
                Ok(())
            }
        }
    }
}
