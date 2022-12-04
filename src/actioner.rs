use tuikit::prelude::{Event, Key, MouseButton};

use crate::fm_error::FmResult;
use crate::keybindings::Keybindings;
use crate::mode::{MarkAction, Mode};
use crate::status::Status;
use crate::term_manager::MIN_WIDTH_FOR_DUAL_PANE;

/// Struct which mutates `tabs.selected()..
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on tabs.selected().
/// Keybindings are read from `Config`.
pub struct Actioner {
    binds: Keybindings,
}

impl Actioner {
    /// Creates a map of configurable keybindings to `EventChar`
    /// The `EventChar` is then associated to a `tabs.selected(). method.
    pub fn new(binds: Keybindings) -> Self {
        Self { binds }
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
            Event::Key(Key::BackTab) => self.backtab(status),
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
            Event::Key(Key::Ctrl('r')) => self.refresh_selected_view(status),
            Event::Key(Key::Ctrl('x')) => self.ctrl_x(status),
            Event::User(_) => self.refresh_selected_view(status),
            Event::Resize { width, height } => self.resize(status, width, height),
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
            | Mode::RegexMatch
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
            Mode::Normal => status.selected().exec_file(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::RegexMatch
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
            | Mode::RegexMatch
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
            | Mode::RegexMatch
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
            Mode::Normal => status.selected().exec_file()?,
            Mode::NeedConfirmation | Mode::Help | Mode::Sort | Mode::Preview | Mode::Marks(_) => (),
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
    /// Select next tab
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

    /// Select previous tab
    fn backtab(&self, status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected().mode {
            status.prev()
        }
        Ok(())
    }

    fn ctrl_f(&self, status: &mut Status) -> FmResult<()> {
        status.create_tabs_from_skim()?;
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

    fn refresh_selected_view(&self, status: &mut Status) -> FmResult<()> {
        status.selected().refresh_view()
    }

    fn ctrl_x(&self, status: &mut Status) -> FmResult<()> {
        status.selected().event_decompress()
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(&self, status: &mut Status, c: char) -> FmResult<()> {
        match status.selected().mode {
            Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename | Mode::Filter => {
                status.selected().event_text_insertion(c);
                Ok(())
            }
            Mode::RegexMatch => {
                status.selected().event_text_insertion(c);
                status.select_from_regex()?;
                Ok(())
            }
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().event_text_insert_and_complete(c)
            }
            Mode::Normal => match self.binds.get(&c) {
                Some(event_char) => event_char.match_char(status),
                None => {
                    if c.is_ascii_digit() {
                        eprintln!("char {} is a digit", c);
                        status.go_tab(c)
                    }
                    Ok(())
                }
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

    fn resize(&self, status: &mut Status, width: usize, height: usize) -> FmResult<()> {
        if width < MIN_WIDTH_FOR_DUAL_PANE {
            status.select_tab(0)?;
            status.set_dual_pane(false);
        } else {
            status.set_dual_pane(true);
        }
        status.selected().set_height(height);
        self.refresh_selected_view(status)?;
        Ok(())
    }
}
