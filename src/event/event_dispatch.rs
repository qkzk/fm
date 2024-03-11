use anyhow::Result;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::app::Status;
use crate::config::Bindings;
use crate::event::event_exec::EventAction;
use crate::modes::{
    Display, Edit, InputCompleted, InputSimple, LeaveMode, MarkAction, Navigate, Search,
};

use super::FmEvents;

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
    pub fn dispatch(&self, status: &mut Status, ev: FmEvents) -> Result<()> {
        match ev {
            FmEvents::Event(Event::Key(key)) => self.match_key_event(status, key),
            FmEvents::Event(Event::Resize { width, height }) => {
                EventAction::resize(status, width, height)
            }
            FmEvents::BulkExecute => EventAction::bulk_confirm(status),
            FmEvents::Refresh => EventAction::refresh_if_needed(status),
            _ => Ok(()),
        }
    }

    fn match_key_event(&self, status: &mut Status, key: Key) -> Result<()> {
        match key {
            Key::WheelUp(row, col, nb_of_scrolls) => {
                EventAction::wheel_up(status, row, col, nb_of_scrolls)?
            }
            Key::WheelDown(row, col, nb_of_scrolls) => {
                EventAction::wheel_down(status, row, col, nb_of_scrolls)?
            }
            Key::SingleClick(MouseButton::Left, row, col) => {
                EventAction::left_click(status, &self.binds, row, col)?
            }
            Key::DoubleClick(MouseButton::Left, row, col) => {
                EventAction::double_click(status, row, col, &self.binds)?
            }
            Key::SingleClick(MouseButton::Right, row, col) => {
                EventAction::left_click(status, &self.binds, row, col)?;
                EventAction::context(status)?
            }

            Key::Char(c) => self.char(status, c)?,
            key => self.key_matcher(status, key)?,
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
        if status.focus.is_file() {
            self.key_matcher(status, Key::Char(c))
        } else {
            let tab = status.current_tab_mut();
            match tab.edit_mode {
                Edit::InputSimple(InputSimple::Sort) => status.sort(c),
                Edit::InputSimple(InputSimple::RegexMatch) => status.input_regex(c),
                Edit::InputSimple(InputSimple::Filter) => status.input_filter(c),
                Edit::InputSimple(_) => status.menu.input_insert(c),
                Edit::InputCompleted(input_completed) => {
                    status.menu.input.insert(c);
                    if matches!(input_completed, InputCompleted::Search) {
                        Self::update_search(status)?;
                        LeaveMode::search(status, false)?
                    }
                    status.menu.input_complete(&mut status.tabs[status.index])?;
                    Ok(())
                }
                Edit::NeedConfirmation(confirmed_action) => status.confirm(c, confirmed_action),
                Edit::Navigate(navigate) => self.navigate_char(navigate, status, c),
                Edit::Nothing if matches!(tab.display_mode, Display::Preview) => {
                    tab.reset_display_mode_and_view()
                }
                Edit::Nothing => self.key_matcher(status, Key::Char(c)),
            }
        }
    }

    fn update_search(status: &mut Status) -> Result<()> {
        if let Ok(search) = Search::new(&status.menu.input.string()) {
            status.current_tab_mut().search = search;
        };
        Ok(())
    }

    fn navigate_char(&self, navigate: Navigate, status: &mut Status, c: char) -> Result<()> {
        match navigate {
            Navigate::Trash if c == 'x' => status.menu.trash_delete_permanently(),
            Navigate::EncryptedDrive if c == 'm' => status.mount_encrypted_drive(),
            Navigate::EncryptedDrive if c == 'g' => status.go_to_encrypted_drive(),
            Navigate::EncryptedDrive if c == 'u' => status.umount_encrypted_drive(),
            Navigate::RemovableDevices if c == 'm' => status.menu.mount_removable(),
            Navigate::RemovableDevices if c == 'g' => status.go_to_removable(),
            Navigate::RemovableDevices if c == 'u' => status.menu.umount_removable(),
            Navigate::Marks(MarkAction::Jump) => status.marks_jump_char(c),
            Navigate::Marks(MarkAction::New) => status.marks_new(c),
            Navigate::Shortcut if status.menu.shortcut_from_char(c) => {
                LeaveMode::leave_edit_mode(status, &self.binds)
            }
            Navigate::Context if status.menu.context_from_char(c) => {
                LeaveMode::leave_edit_mode(status, &self.binds)
            }

            _ => {
                status.reset_edit_mode()?;
                status.current_tab_mut().reset_display_mode_and_view()
            }
        }
    }
}
