use anyhow::Result;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::event_exec::{EventAction, LeaveMode};
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
            Event::Key(Key::AltPageUp) => status.selected().refresh_if_needed()?,

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
        match tab.mode {
            Mode::InputSimple(InputSimple::Sort) => tab.sort(c),
            Mode::InputSimple(InputSimple::RegexMatch) => status.input_regex(c),
            Mode::InputSimple(_) => tab.input_insert(c),
            Mode::InputCompleted(_) => tab.text_insert_and_complete(c),
            Mode::Normal | Mode::Tree => self.key_matcher(status, Key::Char(c)),
            Mode::NeedConfirmation(confirmed_action) => status.confirm(c, confirmed_action),
            Mode::Navigate(Navigate::Trash) if c == 'x' => status.trash_remove(),
            Mode::Navigate(Navigate::EncryptedDrive) if c == 'm' => status.mount_encrypted_drive(),
            Mode::Navigate(Navigate::EncryptedDrive) if c == 'g' => status.go_to_encrypted_drive(),
            Mode::Navigate(Navigate::EncryptedDrive) if c == 'u' => status.umount_encrypted_drive(),
            Mode::Navigate(Navigate::RemovableDevices) if c == 'm' => status.mount_removable(),
            Mode::Navigate(Navigate::RemovableDevices) if c == 'g' => status.go_to_removable(),
            Mode::Navigate(Navigate::RemovableDevices) if c == 'u' => status.umount_removable(),
            Mode::Navigate(Navigate::Jump) if c == ' ' => status.jump_remove_selected_flagged(),
            Mode::Navigate(Navigate::Jump) if c == 'u' => status.clear_flags_and_reset_view(),
            Mode::Navigate(Navigate::Jump) if c == 'x' => status.delete_single_flagged(),
            Mode::Navigate(Navigate::Jump) if c == 'X' => status.trash_single_flagged(),
            Mode::Navigate(Navigate::Marks(MarkAction::Jump)) => status.marks_jump_char(c),
            Mode::Navigate(Navigate::Marks(MarkAction::New)) => status.marks_new(c),
            Mode::Preview | Mode::Navigate(_) => tab.reset_mode_and_view(),
        }
    }
}
