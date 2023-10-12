use anyhow::Result;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::config::Colors;
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
    pub fn dispatch(
        &self,
        status: &mut Status,
        ev: Event,
        colors: &Colors,
        current_height: usize,
    ) -> Result<()> {
        match ev {
            Event::Key(Key::WheelUp(_, col, _)) => {
                status.select_pane(col)?;
                EventAction::move_up(status, colors)?;
            }
            Event::Key(Key::WheelDown(_, col, _)) => {
                status.select_pane(col)?;
                EventAction::move_down(status, colors)?;
            }
            Event::Key(Key::SingleClick(MouseButton::Left, row, col)) => {
                status.click(row, col, current_height, colors)?;
            }
            Event::Key(
                Key::SingleClick(MouseButton::Right, row, col)
                | Key::DoubleClick(MouseButton::Left, row, col),
            ) => {
                status.click(row, col, current_height, colors)?;
                LeaveMode::right_click(status, colors)?;
            }
            Event::User(_) => status.refresh_status(colors)?,
            Event::Resize { width, height } => status.resize(width, height)?,
            Event::Key(Key::Char(c)) => self.char(status, c, colors)?,
            Event::Key(key) => self.key_matcher(status, key, colors)?,
            _ => (),
        };
        if status.dual_pane && status.preview_second {
            status.force_preview(colors)
        } else {
            Ok(())
        }
    }

    fn key_matcher(&self, status: &mut Status, key: Key, colors: &Colors) -> Result<()> {
        match self.binds.get(&key) {
            Some(action) => action.matcher(status, colors),
            None => Ok(()),
        }
    }

    fn char(&self, status: &mut Status, c: char, colors: &Colors) -> Result<()> {
        let tab = status.selected();
        match tab.mode {
            Mode::InputSimple(InputSimple::Sort) => tab.sort(c, colors),
            Mode::InputSimple(InputSimple::RegexMatch) => {
                tab.input.insert(c);
                status.select_from_regex()?;
                Ok(())
            }
            Mode::InputSimple(_) => {
                tab.input.insert(c);
                Ok(())
            }
            Mode::InputCompleted(_) => tab.text_insert_and_complete(c),
            Mode::Normal | Mode::Tree => match self.binds.get(&Key::Char(c)) {
                Some(action) => action.matcher(status, colors),
                None => Ok(()),
            },
            Mode::NeedConfirmation(confirmed_action) => status.confirm(c, confirmed_action, colors),
            Mode::Navigate(Navigate::Trash) if c == 'x' => status.trash.remove(),
            Mode::Navigate(Navigate::EncryptedDrive) if c == 'm' => status.mount_encrypted_drive(),
            Mode::Navigate(Navigate::EncryptedDrive) if c == 'g' => status.go_to_encrypted_drive(),
            Mode::Navigate(Navigate::EncryptedDrive) if c == 'u' => status.umount_encrypted_drive(),
            Mode::Navigate(Navigate::Marks(MarkAction::Jump)) => status.marks_jump_char(c, colors),
            Mode::Navigate(Navigate::Marks(MarkAction::New)) => status.marks_new(c, colors),
            Mode::Preview | Mode::Navigate(_) => {
                if tab.reset_mode() {
                    tab.refresh_view()?;
                }
                Ok(())
            }
        }
    }
}
