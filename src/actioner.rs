use std::cmp::min;

use tuikit::prelude::{Event, Key, MouseButton};

use crate::fileinfo::FileKind;
use crate::mode::Mode;
use crate::status::Status;

pub struct Actioner {}

impl Actioner {
    /// Reaction to received events.
    pub fn read_event(&self, status: &mut Status, ev: Event) {
        match ev {
            Event::Key(Key::ESC) => self.action_escape(status),
            Event::Key(Key::Up) => self.action_up(status),
            Event::Key(Key::Down) => self.action_down(status),
            Event::Key(Key::Left) => self.action_left(status),
            Event::Key(Key::Right) => self.action_right(status),
            Event::Key(Key::Backspace) => self.action_backspace(status),
            Event::Key(Key::Ctrl('d')) => self.action_delete(status),
            Event::Key(Key::Delete) => self.action_delete(status),
            Event::Key(Key::Char(c)) => self.action_char(status, c),
            Event::Key(Key::Home) => self.action_home(status),
            Event::Key(Key::End) => self.action_end(status),
            Event::Key(Key::PageDown) => self.action_page_down(status),
            Event::Key(Key::PageUp) => self.action_page_up(status),
            Event::Key(Key::Enter) => self.action_enter(status),
            Event::Key(Key::Tab) => self.action_tab(status),
            Event::Key(Key::WheelUp(_, _, _)) => self.action_up(status),
            Event::Key(Key::WheelDown(_, _, _)) => self.action_down(status),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => {
                self.action_left_click(status, row)
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                self.action_right_click(status, row)
            }
            _ => {}
        }
    }

    /// Leaving a mode reset the window
    fn action_escape(&self, status: &mut Status) {
        status.event_mode_normal()
    }

    /// Move one line up
    fn action_up(&self, status: &mut Status) {
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
    fn action_down(&self, status: &mut Status) {
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
    fn action_left(&self, status: &mut Status) {
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
    fn action_right(&self, status: &mut Status) {
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
    fn action_backspace(&self, status: &mut Status) {
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

    fn action_delete(&self, status: &mut Status) {
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

    fn action_home(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            status.event_go_top()
        } else {
            status.input_string_cursor_index = 0;
        }
    }

    fn action_end(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            let last_index = status.path_content.files.len() - 1;
            status.path_content.select_index(last_index);
            status.file_index = last_index;
            status.window.scroll_to(last_index);
        } else {
            status.input_string_cursor_index = status.input_string.len();
        }
    }

    fn action_page_down(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            let down_index = min(status.path_content.files.len() - 1, status.file_index + 10);
            status.path_content.select_index(down_index);
            status.file_index = down_index;
            status.window.scroll_to(down_index);
        }
    }

    fn action_page_up(&self, status: &mut Status) {
        if let Mode::Normal = status.mode {
            let up_index = if status.file_index > 10 {
                status.file_index - 10
            } else {
                0
            };
            status.path_content.select_index(up_index);
            status.file_index = up_index;
            status.window.scroll_to(up_index);
        }
    }

    fn action_enter(&self, status: &mut Status) {
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
            Mode::Normal | Mode::NeedConfirmation | Mode::Help | Mode::Sort => (),
        }

        status.input_string_cursor_index = 0;
        status.mode = Mode::Normal;
    }

    fn action_left_click(&self, status: &mut Status, row: u16) {
        if let Mode::Normal = status.mode {
            status.event_select_row(row)
        }
    }

    fn action_right_click(&self, status: &mut Status, row: u16) {
        if let Mode::Normal = status.mode {
            status.file_index = (row - 1).into();
            status.path_content.select_index(status.file_index);
            status.window.scroll_to(status.file_index)
        }
        if let FileKind::Directory = status.path_content.files[status.file_index].file_kind {
            status.event_go_to_child()
        } else {
            status.event_open_file()
        }
    }

    fn action_tab(&self, status: &mut Status) {
        match status.mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.input_string = status.completion.current_proposition()
            }
            _ => (),
        }
    }

    fn action_char(&self, status: &mut Status, c: char) {
        match status.mode {
            Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename | Mode::RegexMatch => {
                status.event_text_insertion(c)
            }
            Mode::Goto | Mode::Exec | Mode::Search => status.event_text_insert_and_complete(c),
            Mode::Normal => {
                if c == status.config.keybindings.toggle_hidden {
                    status.event_toggle_hidden()
                } else if c == status.config.keybindings.copy_paste {
                    status.event_copy_paste()
                } else if c == status.config.keybindings.cut_paste {
                    status.event_cur_paste()
                } else if c == status.config.keybindings.newdir {
                    status.event_new_dir()
                } else if c == status.config.keybindings.newfile {
                    status.event_new_file()
                } else if c == status.config.keybindings.chmod {
                    status.event_chmod()
                } else if c == status.config.keybindings.exec {
                    status.event_exec()
                } else if c == status.config.keybindings.goto {
                    status.event_goto()
                } else if c == status.config.keybindings.rename {
                    status.event_rename()
                } else if c == status.config.keybindings.clear_flags {
                    status.event_clear_flags()
                } else if c == status.config.keybindings.toggle_flag {
                    status.event_toggle_flag()
                } else if c == status.config.keybindings.shell {
                    status.event_shell()
                } else if c == status.config.keybindings.delete {
                    status.event_delete_file()
                } else if c == status.config.keybindings.open_file {
                    status.event_open_file()
                } else if c == status.config.keybindings.help {
                    status.event_help()
                } else if c == status.config.keybindings.search {
                    status.event_search()
                } else if c == status.config.keybindings.regex_match {
                    status.event_regex_match()
                } else if c == status.config.keybindings.quit {
                    status.event_quit()
                } else if c == status.config.keybindings.flag_all {
                    status.event_flag_all()
                } else if c == status.config.keybindings.reverse_flags {
                    status.event_reverse_flags()
                } else if c == status.config.keybindings.jump {
                    status.event_jump();
                } else if c == status.config.keybindings.nvim {
                    status.event_nvim_filepicker()
                } else if c == status.config.keybindings.sort_by {
                    status.event_sort()
                }
            }
            Mode::Help => status.event_normal(),
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
