use anyhow::Result;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::config::Colors;
use crate::event_exec::{
    exec_confirmed_action, exec_marks_jump_char, exec_marks_new, helper_leave_need_confirmation,
    helper_leave_sort, helper_mount_encrypted_drive, helper_move_to_encrypted_drive,
    helper_right_click, helper_select_pane, helper_select_row, helper_text_insert_and_complete,
    helper_text_insertion, helper_trash_remove_file, helper_umount_encrypted_drive, refresh_status,
    resize, EventExec,
};
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
                helper_select_pane(status, col)?;
                EventExec::event_move_up(status, colors)?;
            }
            Event::Key(Key::WheelDown(_, col, _)) => {
                helper_select_pane(status, col)?;
                EventExec::event_move_down(status, colors)?;
            }
            Event::Key(Key::SingleClick(MouseButton::Left, row, col)) => {
                helper_select_pane(status, col)?;
                helper_select_row(status, row, colors)?;
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, col))
            | Event::Key(Key::DoubleClick(MouseButton::Left, row, col)) => {
                helper_select_pane(status, col)?;
                helper_select_row(status, row, colors)?;
                helper_right_click(status, colors)?;
            }
            Event::User(_) => refresh_status(status, colors)?,
            Event::Resize { width, height } => resize(status, width, height, colors)?,
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
                Mode::InputSimple(InputSimple::Sort) => helper_leave_sort(status, c, colors),
                Mode::InputSimple(InputSimple::RegexMatch) => {
                    helper_text_insertion(status.selected(), c);
                    status.select_from_regex()?;
                    Ok(())
                }
                Mode::InputSimple(_) => {
                    helper_text_insertion(status.selected(), c);
                    Ok(())
                }
                Mode::InputCompleted(_) => helper_text_insert_and_complete(status.selected(), c),
                Mode::Normal | Mode::Tree => match self.binds.get(&key_char) {
                    Some(helper_char) => helper_char.matcher(status, colors),
                    None => Ok(()),
                },
                Mode::NeedConfirmation(confirmed_action) => {
                    if c == 'y' {
                        let _ = exec_confirmed_action(status, confirmed_action, colors);
                    }
                    helper_leave_need_confirmation(status.selected());
                    Ok(())
                }
                Mode::Navigate(Navigate::Trash) if c == 'x' => helper_trash_remove_file(status),
                Mode::Navigate(Navigate::EncryptedDrive) if c == 'm' => {
                    helper_mount_encrypted_drive(status)
                }
                Mode::Navigate(Navigate::EncryptedDrive) if c == 'g' => {
                    helper_move_to_encrypted_drive(status)
                }
                Mode::Navigate(Navigate::EncryptedDrive) if c == 'u' => {
                    helper_umount_encrypted_drive(status)
                }
                Mode::Navigate(Navigate::Marks(MarkAction::Jump)) => {
                    exec_marks_jump_char(status, c, colors)
                }
                Mode::Navigate(Navigate::Marks(MarkAction::New)) => {
                    exec_marks_new(status, c, colors)
                }
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
