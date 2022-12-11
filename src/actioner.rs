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
            Event::Key(Key::WheelUp(_, _, _)) => EventExec::event_move_up(status),
            Event::Key(Key::WheelDown(_, _, _)) => EventExec::event_move_down(status),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => {
                EventExec::event_select_row(status, row)
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                EventExec::event_right_click(status, row)
            }
            Event::User(_) => EventExec::refresh_selected_view(status),
            Event::Resize { width, height } => EventExec::resize(status, width, height),
            Event::Key(Key::Char(c)) => self.char(status, Key::Char(c)),
            Event::Key(key) => self.key_matcher(status, key),
            _ => Ok(()),
        }
    }

    fn key_matcher(&self, status: &mut Status, key: Key) -> FmResult<()> {
        match self.binds.get(&key) {
            Some(action) => action.matcher(status),
            None => Ok(()),
        }
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(&self, status: &mut Status, key_char: Key) -> FmResult<()> {
        match key_char {
            Key::Char(c) => match status.selected_non_mut().mode {
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
                Mode::Normal => match self.binds.get(&key_char) {
                    Some(event_char) => event_char.matcher(status),
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
