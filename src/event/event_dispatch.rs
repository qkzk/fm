use anyhow::Result;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::app::Status;
use crate::config::{Bindings, REFRESH_EVENT};
use crate::event::event_exec::{EventAction, LeaveMode};
use crate::modes::{DisplayMode, EditMode, InputSimple, MarkAction, Navigate};

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
    pub fn dispatch(&self, status: &mut Status, ev: Event, current_height: usize) -> Result<()> {
        match ev {
            Event::Key(Key::WheelUp(_, col, _)) => {
                status.select_pane(col)?;
                EventAction::move_up(status)?;
            }
            Event::Key(Key::WheelDown(_, col, _)) => {
                status.select_pane(col)?;
                EventAction::move_down(status)?;
            }
            Event::Key(Key::SingleClick(MouseButton::Left, row, col)) => {
                status.click(row, col, current_height)?;
            }
            Event::Key(
                Key::SingleClick(MouseButton::Right, row, col)
                | Key::DoubleClick(MouseButton::Left, row, col),
            ) => {
                status.click(row, col, current_height)?;
                LeaveMode::right_click(status)?;
            }
            // reserved keybind which can't be bound to anything.
            // using `Key::User(())` conflicts with skim internal which
            // interpret this event as a signal(1)
            REFRESH_EVENT => status.selected().refresh_if_needed()?,

            Event::Resize { width, height } => status.resize(width, height)?,
            Event::Key(Key::Char(c)) => self.char(status, c)?,
            Event::Key(key) => self.key_matcher(status, key)?,
            _ => (),
        };
        Ok(())
    }

    fn key_matcher(&self, status: &mut Status, key: Key) -> Result<()> {
        match self.binds.get(&key) {
            Some(action) => action.matcher(status),
            None => Ok(()),
        }
    }

    fn char(&self, status: &mut Status, c: char) -> Result<()> {
        let tab = status.selected();
        match tab.edit_mode {
            EditMode::InputSimple(InputSimple::Sort) => tab.sort(c),
            EditMode::InputSimple(InputSimple::RegexMatch) => status.input_regex(c),
            EditMode::InputSimple(_) => tab.input_insert(c),
            EditMode::InputCompleted(_) => tab.text_insert_and_complete(c),
            EditMode::NeedConfirmation(confirmed_action) => status.confirm(c, confirmed_action),
            EditMode::Navigate(Navigate::Trash) if c == 'x' => status.trash_delete_permanently(),
            EditMode::Navigate(Navigate::EncryptedDrive) if c == 'm' => {
                status.mount_encrypted_drive()
            }
            EditMode::Navigate(Navigate::EncryptedDrive) if c == 'g' => {
                status.go_to_encrypted_drive()
            }
            EditMode::Navigate(Navigate::EncryptedDrive) if c == 'u' => {
                status.umount_encrypted_drive()
            }
            EditMode::Navigate(Navigate::RemovableDevices) if c == 'm' => status.mount_removable(),
            EditMode::Navigate(Navigate::RemovableDevices) if c == 'g' => status.go_to_removable(),
            EditMode::Navigate(Navigate::RemovableDevices) if c == 'u' => status.umount_removable(),
            EditMode::Navigate(Navigate::Jump) if c == ' ' => status.jump_remove_selected_flagged(),
            EditMode::Navigate(Navigate::Jump) if c == 'u' => status.clear_flags_and_reset_view(),
            EditMode::Navigate(Navigate::Jump) if c == 'x' => status.delete_single_flagged(),
            EditMode::Navigate(Navigate::Jump) if c == 'X' => status.trash_single_flagged(),
            EditMode::Navigate(Navigate::Marks(MarkAction::Jump)) => status.marks_jump_char(c),
            EditMode::Navigate(Navigate::Marks(MarkAction::New)) => status.marks_new(c),
            EditMode::Navigate(_) => tab.reset_mode_and_view(),
            EditMode::Nothing if matches!(tab.display_mode, DisplayMode::Preview) => {
                tab.reset_mode_and_view()
            }
            EditMode::Nothing => self.key_matcher(status, Key::Char(c)),
        }
    }
}
