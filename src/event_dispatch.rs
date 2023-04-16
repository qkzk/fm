use anyhow::Result;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::config::Colors;
use crate::event_exec::{EventAction, LeaveMode};
use crate::keybindings::Bindings;
use crate::mode::{InputSimple, MarkAction, Mode, Navigate};
use crate::status::Status;
use crate::tab::Tab;

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
    pub fn dispatch(&self, status: &mut Status, ev: Event, colors: &Colors) -> Result<()> {
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
                status.select_pane(col)?;
                status.selected().select_row(row, colors)?;
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, col))
            | Event::Key(Key::DoubleClick(MouseButton::Left, row, col)) => {
                status.select_pane(col)?;
                status.selected().select_row(row, colors)?;
                LeaveMode::right_click(status, colors)?;
            }
            Event::User(_) => status.refresh_status(colors)?,
            Event::Resize { width, height } => status.resize(width, height, colors)?,
            Event::Key(Key::Char(c)) => self.char(status, Key::Char(c), colors)?,
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

    fn char(&self, status: &mut Status, key_char: Key, colors: &Colors) -> Result<()> {
        match key_char {
            Key::Char(c) => match status.selected_non_mut().mode {
                Mode::InputSimple(InputSimple::Sort) => status.selected().sort(c, colors),
                Mode::InputSimple(InputSimple::RegexMatch) => {
                    {
                        let tab: &mut Tab = status.selected();
                        tab.input.insert(c);
                    };
                    status.select_from_regex()?;
                    Ok(())
                }
                Mode::InputSimple(_) => {
                    {
                        let tab: &mut Tab = status.selected();
                        tab.input.insert(c);
                    };
                    Ok(())
                }
                Mode::InputCompleted(_) => status.selected().text_insert_and_complete(c),
                Mode::Normal | Mode::Tree => match self.binds.get(&key_char) {
                    Some(char) => char.matcher(status, colors),
                    None => Ok(()),
                },
                Mode::NeedConfirmation(confirmed_action) => {
                    if c == 'y' {
                        let _ = status.confirm_action(confirmed_action, colors);
                    }
                    status.selected().reset_mode();
                    Ok(())
                }
                Mode::Navigate(Navigate::Trash) if c == 'x' => status.trash.remove(),
                Mode::Navigate(Navigate::EncryptedDrive) if c == 'm' => {
                    status.mount_encrypted_drive()
                }
                Mode::Navigate(Navigate::EncryptedDrive) if c == 'g' => {
                    status.move_to_encrypted_drive()
                }
                Mode::Navigate(Navigate::EncryptedDrive) if c == 'u' => {
                    status.umount_encrypted_drive()
                }
                Mode::Navigate(Navigate::Marks(MarkAction::Jump)) => {
                    status.marks_jump_char(c, colors)
                }
                Mode::Navigate(Navigate::Marks(MarkAction::New)) => status.marks_new(c, colors),
                Mode::Preview | Mode::Navigate(_) => {
                    status.selected().set_mode(Mode::Normal);
                    {
                        let tab: &mut Tab = status.selected();
                        tab.refresh_view()
                    }
                }
            },
            _ => Ok(()),
        }
    }
}
