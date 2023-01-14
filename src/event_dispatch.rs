use tuikit::prelude::{Event, Key, MouseButton};

use crate::event_exec::EventExec;
use crate::fm_error::FmResult;
use crate::keybindings::Bindings;
use crate::mode::{InputSimple, MarkAction, Mode, Navigate};
use crate::status::Status;

/// Struct which mutates `tabs.selected()..
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on tabs.selected().
/// Keybindings are read from `Config`.
pub struct EventDispatcher {
    binds: Bindings,
}

impl EventDispatcher {
    /// Creates a map of configurable keybindings to `EventChar`
    /// The `EventChar` is then associated to a `tabs.selected(). method.
    pub fn new(binds: Bindings) -> Self {
        Self { binds }
    }

    /// Reaction to received events.
    /// Only non keyboard events are dealt here directly.
    /// Keyboard events are configurable and are sent to specific functions
    /// which needs to know those keybindings.
    pub fn dispatch(&self, status: &mut Status, ev: Event) -> FmResult<()> {
        match ev {
            Event::Key(Key::WheelUp(_, col, _)) => {
                EventExec::event_select_pane(status, col)?;
                EventExec::event_move_up(status)
            }
            Event::Key(Key::WheelDown(_, col, _)) => {
                EventExec::event_select_pane(status, col)?;
                EventExec::event_move_down(status)
            }
            Event::Key(Key::SingleClick(MouseButton::Left, row, col)) => {
                EventExec::event_select_pane(status, col)?;
                EventExec::event_select_row(status, row)
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, col))
            | Event::Key(Key::DoubleClick(MouseButton::Left, row, col)) => {
                EventExec::event_select_pane(status, col)?;
                EventExec::event_right_click(status, row)
            }
            Event::User(_) => EventExec::refresh_status(status),
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

    fn char(&self, status: &mut Status, key_char: Key) -> FmResult<()> {
        match key_char {
            Key::Char(c) => match status.selected_non_mut().mode {
                Mode::InputSimple(InputSimple::Marks(MarkAction::Jump)) => {
                    EventExec::exec_marks_jump(status, c)
                }
                Mode::InputSimple(InputSimple::Marks(MarkAction::New)) => {
                    EventExec::exec_marks_new(status, c)
                }
                Mode::InputSimple(InputSimple::Sort) => EventExec::event_leave_sort(status, c),
                Mode::InputSimple(InputSimple::RegexMatch) => {
                    EventExec::event_text_insertion(status.selected(), c);
                    status.select_from_regex()?;
                    Ok(())
                }
                Mode::InputSimple(_) => {
                    EventExec::event_text_insertion(status.selected(), c);
                    Ok(())
                }
                Mode::InputCompleted(_) => {
                    EventExec::event_text_insert_and_complete(status.selected(), c)
                }
                Mode::Normal | Mode::Tree => match self.binds.get(&key_char) {
                    Some(event_char) => event_char.matcher(status),
                    None => Ok(()),
                },
                Mode::NeedConfirmation(confirmed_action) => {
                    if c == 'y' {
                        let _ = EventExec::exec_confirmed_action(status, confirmed_action);
                    }
                    EventExec::event_leave_need_confirmation(status.selected());
                    Ok(())
                }
                Mode::Navigate(Navigate::Trash) if c == 'x' => {
                    EventExec::event_trash_remove_file(status)
                }
                Mode::Preview | Mode::Navigate(_) => EventExec::event_normal(status.selected()),
            },
            _ => Ok(()),
        }
    }
}
