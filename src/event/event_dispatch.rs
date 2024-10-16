use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::app::Status;
use crate::config::Bindings;
use crate::event::{EventAction, FmEvents};
use crate::modes::{Display, InputCompleted, InputSimple, LeaveMenu, MarkAction, Menu, Navigate};

/// Struct which mutates `tabs.selected()..
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on tabs.selected().
/// Keybindings are read from `Config`.
pub struct EventDispatcher {
    binds: Bindings,
}

impl EventDispatcher {
    /// Creates a new event dispatcher with those bindings.
    pub fn new(binds: Bindings) -> Self {
        Self { binds }
    }

    /// Reaction to received events.
    /// Only non keyboard events are dealt here directly.
    /// Keyboard events are configurable and are sent to specific functions
    /// which needs to know those keybindings.
    pub fn dispatch(&self, status: &mut Status, ev: FmEvents) -> Result<()> {
        match ev {
            FmEvents::Term(Event::Key(key)) => self.match_key_event(status, key),
            FmEvents::Term(Event::Mouse(mouse)) => self.match_mouse_event(status, mouse),
            FmEvents::Term(Event::Resize(width, height)) => {
                EventAction::resize(status, width as usize, height as usize)
            }
            FmEvents::BulkExecute => EventAction::bulk_confirm(status),
            FmEvents::Refresh => EventAction::refresh_if_needed(status),
            FmEvents::FileCopied => EventAction::file_copied(status),
            FmEvents::CheckPreview => EventAction::check_preview(status),
            FmEvents::Action(action) => action.matcher(status, &self.binds),
            _ => Ok(()),
        }
    }

    fn match_key_event(&self, status: &mut Status, key: KeyEvent) -> Result<()> {
        match key {
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: _,
                kind: _,
                state: _,
            } if !status.focus.is_file() => self.menu_key_matcher(status, c)?,
            key => self.file_key_matcher(status, key)?,
        };
        Ok(())
    }

    fn match_mouse_event(&self, status: &mut Status, mouse_event: MouseEvent) -> Result<()> {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                EventAction::wheel_up(status, mouse_event.row, mouse_event.column, 1)
            }
            MouseEventKind::ScrollDown => {
                EventAction::wheel_down(status, mouse_event.row, mouse_event.column, 1)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                EventAction::left_click(status, &self.binds, mouse_event.row, mouse_event.column)
            }
            // TODO! doubleclick
            // MouseEventKind::Up(MouseButton::Left, row, col) => {
            //     EventAction::double_click(status, row, col, &self.binds)
            // }
            MouseEventKind::Up(MouseButton::Right) => {
                EventAction::right_click(status, &self.binds, mouse_event.row, mouse_event.column)
            }
            _ => unreachable!("{mouse_event:?} should be a mouse event"),
        }
    }

    fn file_key_matcher(&self, status: &mut Status, key: KeyEvent) -> Result<()> {
        let Some(action) = self.binds.get(&key) else {
            return Ok(());
        };
        action.matcher(status, &self.binds)
    }

    fn menu_key_matcher(&self, status: &mut Status, c: char) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.menu_mode {
            Menu::InputSimple(InputSimple::Sort) => status.sort(c),
            Menu::InputSimple(InputSimple::RegexMatch) => status.input_regex(c),
            Menu::InputSimple(InputSimple::Filter) => status.input_filter(c),
            Menu::InputSimple(_) => status.menu.input_insert(c),
            Menu::InputCompleted(InputCompleted::Search) => status.complete_search(c),
            Menu::InputCompleted(_) => status.complete_non_search(c),
            Menu::NeedConfirmation(confirmed_action) => status.confirm(c, confirmed_action),
            Menu::Navigate(navigate) => self.navigate_char(navigate, status, c),
            _ if matches!(tab.display_mode, Display::Preview) => tab.reset_display_mode_and_view(),
            Menu::Nothing => unreachable!("Focus can't be in menu if menu is Nothing"),
        }
    }

    fn navigate_char(&self, navigate: Navigate, status: &mut Status, c: char) -> Result<()> {
        match navigate {
            Navigate::Trash if c == 'x' => status.menu.trash_delete_permanently(),

            Navigate::EncryptedDrive if c == 'm' => status.mount_encrypted_drive(),
            Navigate::EncryptedDrive if c == 'g' => status.go_to_encrypted_drive(),
            Navigate::EncryptedDrive if c == 'u' => status.umount_encrypted_drive(),

            Navigate::RemovableDevices if c == 'm' => status.mount_removable(),
            Navigate::RemovableDevices if c == 'g' => status.go_to_removable(),
            Navigate::RemovableDevices if c == 'u' => status.umount_removable(),

            Navigate::Marks(MarkAction::Jump) => status.marks_jump_char(c),
            Navigate::Marks(MarkAction::New) => status.marks_new(c),

            Navigate::Shortcut if status.menu.shortcut_from_char(c) => {
                LeaveMenu::leave_edit_mode(status, &self.binds)
            }

            Navigate::Context if status.menu.context_from_char(c) => {
                LeaveMenu::leave_edit_mode(status, &self.binds)
            }

            Navigate::Cloud if c == 'l' => status.cloud_disconnect(),
            Navigate::Cloud if c == 'd' => status.cloud_enter_newdir_mode(),
            Navigate::Cloud if c == 'u' => status.cloud_upload_selected_file(),
            Navigate::Cloud if c == 'x' => status.cloud_enter_delete_mode(),
            Navigate::Cloud if c == '?' => status.cloud_update_metadata(),

            Navigate::Flagged if c == 'u' => {
                status.menu.flagged.clear();
                Ok(())
            }
            Navigate::Flagged if c == 'x' => status.menu.remove_selected_flagged(),
            Navigate::Flagged if c == 'j' => status.jump_flagged(),

            _ => {
                status.reset_menu_mode()?;
                status.current_tab_mut().reset_display_mode_and_view()
            }
        }
    }
}
