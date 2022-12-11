use tuikit::prelude::{Event, Key, MouseButton};

use crate::event_exec::EventExec;
use crate::fm_error::FmResult;
use crate::keybindings::Bindings;
use crate::mode::{MarkAction, Mode};
use crate::status::Status;

/// Struct which mutates `tabs.selected()..
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on tabs.selected().
/// Keybindings are read from `Config`.
pub struct Actioner {
    binds: Bindings,
}

impl Actioner {
    /// Creates a map of configurable keybindings to `EventChar`
    /// The `EventChar` is then associated to a `tabs.selected(). method.
    pub fn new(binds: Bindings) -> Self {
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
            Event::Key(Key::Char(c)) => Self::char(status, Key::Char(c), &self.binds),
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
            Event::Key(Key::Ctrl('r')) => Self::ctrl_r(status),
            Event::Key(Key::Ctrl('x')) => Self::ctrl_x(status),
            Event::Key(Key::Ctrl('e')) => Self::ctrl_e(status),
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
        EventExec::event_move_up(status)
    }

    /// Move one line down
    fn down(status: &mut Status) -> FmResult<()> {
        EventExec::event_move_down(status)
    }

    /// Move left in a string, move to parent in normal mode
    fn left(status: &mut Status) -> FmResult<()> {
        EventExec::event_move_left(status)
    }

    /// Move right in a string, move to children in normal mode.
    fn right(status: &mut Status) -> FmResult<()> {
        EventExec::event_move_right(status)
    }

    /// Deletes a char in input string
    fn backspace(status: &mut Status) -> FmResult<()> {
        EventExec::event_backspace(status)
    }

    /// Deletes chars right of cursor in input string.
    /// Remove current tab in normal mode.
    fn delete(status: &mut Status) -> FmResult<()> {
        EventExec::event_delete(status)
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
        EventExec::event_end(status)
    }

    /// Move down 10 rows
    fn page_down(status: &mut Status) -> FmResult<()> {
        EventExec::page_down(status)
    }

    /// Move up 10 rows
    fn page_up(status: &mut Status) -> FmResult<()> {
        EventExec::page_up(status)
    }

    /// Execute a command
    fn enter(status: &mut Status) -> FmResult<()> {
        EventExec::enter(status)
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
        EventExec::tab(status)
    }

    /// Select previous tab
    fn backtab(status: &mut Status) -> FmResult<()> {
        EventExec::backtab(status)
    }

    fn ctrl_f(status: &mut Status) -> FmResult<()> {
        EventExec::ctrl_f(status)
    }

    fn ctrl_c(status: &mut Status) -> FmResult<()> {
        EventExec::ctrl_c(status)
    }

    fn ctrl_p(status: &mut Status) -> FmResult<()> {
        EventExec::ctrl_p(status)
    }

    fn ctrl_r(status: &mut Status) -> FmResult<()> {
        EventExec::ctrl_r(status)
    }

    fn ctrl_x(status: &mut Status) -> FmResult<()> {
        EventExec::ctrl_x(status)
    }

    fn ctrl_e(status: &mut Status) -> FmResult<()> {
        EventExec::ctrl_e(status)
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(status: &mut Status, k: Key, binds: &Bindings) -> FmResult<()> {
        match k {
            Key::Char(c) => match status.selected().mode {
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
                Mode::Normal => match binds.get(&k) {
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
            },
            _ => Ok(()),
        }
    }
}
