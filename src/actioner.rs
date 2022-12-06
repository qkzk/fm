use tuikit::prelude::{Event, Key, MouseButton};

use crate::event_exec::EventExec;
use crate::fm_error::FmResult;
use crate::keybindings::Keybindings;
use crate::mode::{MarkAction, Mode};
use crate::status::Status;

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
            Event::Key(Key::ESC) => Self::escape(status),
            Event::Key(Key::Up) => Self::up(status),
            Event::Key(Key::Down) => Self::down(status),
            Event::Key(Key::Left) => Self::left(status),
            Event::Key(Key::Right) => Self::right(status),
            Event::Key(Key::Backspace) => Self::backspace(status),
            Event::Key(Key::Ctrl('d')) => Self::delete(status),
            Event::Key(Key::Ctrl('q')) => Self::escape(status),
            Event::Key(Key::Char(c)) => Self::char(status, c, &self.binds),
            Event::Key(Key::Home) => Self::home(status),
            Event::Key(Key::End) => Self::end(status),
            Event::Key(Key::PageDown) => Self::page_down(status),
            Event::Key(Key::PageUp) => Self::page_up(status),
            Event::Key(Key::Enter) => Self::enter(status),
            Event::Key(Key::Tab) => Self::tab(status),
            Event::Key(Key::BackTab) => Self::backtab(status),
            Event::Key(Key::WheelUp(_, _, _)) => Self::up(status),
            Event::Key(Key::WheelDown(_, _, _)) => Self::down(status),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => {
                Self::left_click(status, row);
                Ok(())
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                Self::right_click(status, row);
                Ok(())
            }
            Event::Key(Key::Ctrl('f')) => Self::ctrl_f(status),
            Event::Key(Key::Ctrl('c')) => Self::ctrl_c(status),
            Event::Key(Key::Ctrl('p')) => Self::ctrl_p(status),
            Event::Key(Key::Ctrl('r')) => EventExec::refresh_selected_view(status),
            Event::Key(Key::Ctrl('x')) => Self::ctrl_x(status),
            Event::User(_) => EventExec::refresh_selected_view(status),
            Event::Resize { width, height } => EventExec::resize(status, width, height),
            _ => Ok(()),
        }
    }

    /// Leaving a mode reset the window
    fn escape(status: &mut Status) -> FmResult<()> {
        EventExec::event_normal(status.selected())
    }

    /// Move one line up
    fn up(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => {
                EventExec::event_up_one_row(status.selected())
            }
            Mode::Jump => EventExec::event_jumplist_prev(status),
            Mode::History => EventExec::event_history_prev(status.selected()),
            Mode::Shortcut => EventExec::event_shortcut_prev(status.selected()),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().completion.prev();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move one line down
    fn down(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => {
                EventExec::event_down_one_row(status.selected())
            }
            Mode::Jump => EventExec::event_jumplist_next(status),
            Mode::History => EventExec::event_history_next(status.selected()),
            Mode::Shortcut => EventExec::event_shortcut_next(status.selected()),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().completion.next();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move left in a string, move to parent in normal mode
    fn left(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => EventExec::event_move_to_parent(status.selected()),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::RegexMatch
            | Mode::Filter => {
                EventExec::event_move_cursor_left(status.selected());
                Ok(())
            }

            _ => Ok(()),
        }
    }

    /// Move right in a string, move to children in normal mode.
    fn right(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => EventExec::exec_file(status.selected()),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::RegexMatch
            | Mode::Filter => {
                EventExec::event_move_cursor_right(status.selected());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Deletes a char in input string
    fn backspace(status: &mut Status) -> FmResult<()> {
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
                EventExec::event_delete_char_left(status.selected());
                Ok(())
            }
            Mode::Normal => Ok(()),
            _ => Ok(()),
        }
    }

    /// Deletes chars right of cursor in input string.
    /// Remove current tab in normal mode.
    fn delete(status: &mut Status) -> FmResult<()> {
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
                EventExec::event_delete_chars_right(status.selected());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Move to top or beggining of line.
    fn home(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => EventExec::event_go_top(status.selected()),
            _ => EventExec::event_cursor_home(status.selected()),
        };
        Ok(())
    }

    /// Move to end or end of line.
    fn end(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => {
                EventExec::event_go_bottom(status.selected())
            }
            _ => EventExec::event_cursor_end(status.selected()),
        };
        Ok(())
    }

    /// Move down 10 rows
    fn page_down(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => {
                EventExec::event_page_down(status.selected())
            }
            _ => (),
        };
        Ok(())
    }

    /// Move up 10 rows
    fn page_up(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => {
                EventExec::event_page_up(status.selected())
            }
            _ => (),
        };
        Ok(())
    }

    /// Execute a command
    fn enter(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Rename => EventExec::exec_rename(status.selected())?,
            Mode::Newfile => EventExec::exec_newfile(status.selected())?,
            Mode::Newdir => EventExec::exec_newdir(status.selected())?,
            Mode::Chmod => EventExec::exec_chmod(status)?,
            Mode::Exec => EventExec::exec_exec(status.selected())?,
            Mode::Search => EventExec::exec_search(status.selected()),
            Mode::Goto => EventExec::exec_goto(status.selected())?,
            Mode::RegexMatch => EventExec::exec_regex(status)?,
            Mode::Jump => EventExec::exec_jump(status)?,
            Mode::History => EventExec::exec_history(status.selected())?,
            Mode::Shortcut => EventExec::exec_shortcut(status.selected())?,
            Mode::Filter => EventExec::exec_filter(status.selected())?,
            Mode::Normal => EventExec::exec_file(status.selected())?,
            Mode::NeedConfirmation | Mode::Help | Mode::Sort | Mode::Preview | Mode::Marks(_) => (),
        };

        status.selected().input.reset();
        status.selected().mode = Mode::Normal;
        Ok(())
    }

    /// Select this file
    fn left_click(status: &mut Status, row: u16) {
        if let Mode::Normal = status.selected().mode {
            EventExec::event_select_row(status.selected(), row)
        }
    }

    /// Open a directory or a file
    fn right_click(status: &mut Status, row: u16) {
        if let Mode::Normal = status.selected().mode {
            let _ = EventExec::event_right_click(status.selected(), row);
        }
    }

    /// Select next completion and insert it
    /// Select next tab
    fn tab(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                EventExec::event_replace_input_with_completion(status.selected())
            }
            Mode::Normal => status.next(),
            _ => (),
        };
        Ok(())
    }

    /// Select previous tab
    fn backtab(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected().mode {
            status.prev()
        }
        Ok(())
    }

    fn ctrl_f(status: &mut Status) -> FmResult<()> {
        status.create_tabs_from_skim()?;
        Ok(())
    }

    fn ctrl_c(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return EventExec::event_filename_to_clipboard(status.selected());
        }
        Ok(())
    }

    fn ctrl_p(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return EventExec::event_filepath_to_clipboard(status.selected());
        }
        Ok(())
    }

    fn ctrl_x(status: &mut Status) -> FmResult<()> {
        EventExec::event_decompress(status.selected())
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(status: &mut Status, c: char, binds: &Keybindings) -> FmResult<()> {
        match status.selected().mode {
            Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename | Mode::Filter => {
                EventExec::event_text_insertion(status.selected(), c);
                Ok(())
            }
            Mode::RegexMatch => {
                EventExec::event_text_insertion(status.selected(), c);
                status.select_from_regex()?;
                Ok(())
            }
            Mode::Goto | Mode::Exec | Mode::Search => {
                EventExec::event_text_insert_and_complete(status.selected(), c)
            }
            Mode::Normal => match binds.get(&c) {
                Some(event_char) => event_char.match_char(status),
                None => Ok(()),
            },
            Mode::Help | Mode::Preview | Mode::Shortcut => {
                EventExec::event_normal(status.selected())
            }
            Mode::Jump => Ok(()),
            Mode::History => Ok(()),
            Mode::NeedConfirmation => {
                if c == 'y' {
                    let _ = EventExec::exec_last_edition(status);
                }
                EventExec::event_leave_need_confirmation(status.selected());
                Ok(())
            }
            Mode::Marks(MarkAction::Jump) => EventExec::exec_marks_jump(status, c),
            Mode::Marks(MarkAction::New) => EventExec::exec_marks_new(status, c),
            Mode::Sort => {
                EventExec::event_leave_sort(status.selected(), c);
                Ok(())
            }
        }
    }
}
