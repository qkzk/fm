use anyhow::Result;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::app::Status;
use crate::config::{Bindings, REFRESH_EVENT};
use crate::event::event_exec::{EventAction, LeaveMode};
use crate::modes::{ContentWindow, Display, Edit, InputSimple, MarkAction, Navigate};

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
    pub fn dispatch(&self, status: &mut Status, ev: Event) -> Result<()> {
        match ev {
            Event::Key(Key::WheelUp(_, col, _)) => {
                EventAction::select_pane(status, col)?;
                EventAction::move_up(status)?;
            }
            Event::Key(Key::WheelDown(_, col, _)) => {
                EventAction::select_pane(status, col)?;
                EventAction::move_down(status)?;
            }
            Event::Key(Key::SingleClick(MouseButton::Left, row, col)) => {
                EventAction::select_pane(status, col)?;
                if row < ContentWindow::HEADER_ROWS as u16 {
                    EventAction::click_first_line(col, status, &self.binds)?;
                } else {
                    let _ = EventAction::click_files(status, row, col);
                }
            }
            Event::Key(
                Key::SingleClick(MouseButton::Right, row, col)
                | Key::DoubleClick(MouseButton::Left, row, col),
            ) => {
                if let Ok(()) = EventAction::click_files(status, row, col) {
                    LeaveMode::right_click(status)?;
                }
            }
            // reserved keybind which can't be bound to anything.
            // using `Key::User(())` conflicts with skim internal which
            // interpret this event as a signal(1)
            REFRESH_EVENT => EventAction::refresh_if_needed(status.current_tab_mut())?,

            Event::Resize { width, height } => EventAction::resize(status, width, height)?,
            Event::Key(Key::Char(c)) => self.char(status, c)?,
            Event::Key(key) => self.key_matcher(status, key)?,
            _ => (),
        };
        Ok(())
    }

    fn key_matcher(&self, status: &mut Status, key: Key) -> Result<()> {
        match self.binds.get(&key) {
            Some(action) => action.matcher(status, &self.binds),
            None => Ok(()),
        }
    }

    fn char(&self, status: &mut Status, c: char) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.edit_mode {
            Edit::InputSimple(InputSimple::Sort) => tab.sort(c),
            Edit::InputSimple(InputSimple::RegexMatch) => status.input_regex(c),
            Edit::InputSimple(_) => status.menu.input_insert(c),
            Edit::InputCompleted(_) => status.text_insert_and_complete(c),
            Edit::NeedConfirmation(confirmed_action) => status.confirm(c, confirmed_action),
            Edit::Navigate(Navigate::Trash) if c == 'x' => status.menu.trash_delete_permanently(),
            Edit::Navigate(Navigate::EncryptedDrive) if c == 'm' => status.mount_encrypted_drive(),
            Edit::Navigate(Navigate::EncryptedDrive) if c == 'g' => status.go_to_encrypted_drive(),
            Edit::Navigate(Navigate::EncryptedDrive) if c == 'u' => status.umount_encrypted_drive(),
            Edit::Navigate(Navigate::RemovableDevices) if c == 'm' => status.menu.mount_removable(),
            Edit::Navigate(Navigate::RemovableDevices) if c == 'g' => status.go_to_removable(),
            Edit::Navigate(Navigate::RemovableDevices) if c == 'u' => {
                status.menu.umount_removable()
            }
            Edit::Navigate(Navigate::Jump) if c == ' ' => status.menu.remove_selected_flagged(),
            Edit::Navigate(Navigate::Jump) if c == 'u' => status.clear_flags_and_reset_view(),
            Edit::Navigate(Navigate::Jump) if c == 'x' => status.menu.delete_single_flagged(),
            Edit::Navigate(Navigate::Jump) if c == 'X' => status.menu.trash_single_flagged(),
            Edit::Navigate(Navigate::Marks(MarkAction::Jump)) => status.marks_jump_char(c),
            Edit::Navigate(Navigate::Marks(MarkAction::New)) => status.marks_new(c),
            Edit::Navigate(_) => tab.reset_mode_and_view(),
            Edit::Nothing if matches!(tab.display_mode, Display::Preview) => {
                tab.reset_mode_and_view()
            }
            Edit::Nothing => self.key_matcher(status, Key::Char(c)),
        }
    }
}
