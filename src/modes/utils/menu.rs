use std::sync::mpsc::Sender;
use std::sync::Arc;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use tuikit::prelude::Canvas;

use crate::app::Tab;
use crate::common::{index_from_a, CLI_PATH, INPUT_HISTORY_PATH, MARKS_FILEPATH, TUIS_PATH};
use crate::config::Bindings;
use crate::event::FmEvents;
use crate::io::DrawMenu;
use crate::io::{drop_sudo_privileges, InputHistory, OpendalContainer};
use crate::log_line;
use crate::modes::{
    Bulk, CLApplications, CliApplications, Completion, Compresser, Content, ContentWindow,
    ContextMenu, CryptoDeviceOpener, Display, Edit, Flagged, History, Input, InputCompleted,
    IsoDevice, Marks, MountCommands, Navigate, PasswordHolder, Picker, Remote, RemovableDevices,
    Selectable, Shortcut, Trash, TuiApplications,
};

pub struct Menu {
    /// Window for scrollable menus
    pub window: ContentWindow,
    /// Bulk rename
    pub bulk: Bulk,
    /// CLI applications
    pub cli_applications: CliApplications,
    /// cloud
    pub cloud: OpendalContainer,
    /// Completion list and index in it.
    pub completion: Completion,
    /// Compression methods
    pub compression: Compresser,
    /// Cotext menu
    pub context: ContextMenu,
    /// Encrypted devices opener
    pub encrypted_devices: CryptoDeviceOpener,
    /// The flagged files
    pub flagged: Flagged,
    /// The typed input by the user
    pub input: Input,
    /// The user input history.
    pub input_history: InputHistory,
    /// Iso mounter. Set to None by default, dropped ASAP
    pub iso_device: Option<IsoDevice>,
    /// Marks allows you to jump to a save mark
    pub marks: Marks,
    /// Hold password between their typing and usage
    pub password_holder: PasswordHolder,
    /// basic picker
    pub picker: Picker,
    /// MTP devices
    pub removable_devices: RemovableDevices,
    /// Predefined shortcuts
    pub shortcut: Shortcut,
    /// TUI application
    pub tui_applications: TuiApplications,
    /// The trash
    pub trash: Trash,
    /// Last sudo command ran
    pub sudo_command: Option<String>,
    /// History - here for compatibility reasons only
    pub history: History,
}

impl Menu {
    pub fn new(
        start_dir: &std::path::Path,
        mount_points: &[&std::path::Path],
        binds: &Bindings,
        fm_sender: Arc<Sender<FmEvents>>,
    ) -> Result<Self> {
        Ok(Self {
            bulk: Bulk::new(fm_sender),
            cli_applications: CliApplications::new(CLI_PATH).update_desc_size(),
            cloud: OpendalContainer::default(),
            completion: Completion::default(),
            compression: Compresser::default(),
            context: ContextMenu::default(),
            encrypted_devices: CryptoDeviceOpener::default(),
            flagged: Flagged::default(),
            history: History::default(),
            input: Input::default(),
            input_history: InputHistory::load(INPUT_HISTORY_PATH)?,
            iso_device: None,
            marks: Marks::new(MARKS_FILEPATH),
            password_holder: PasswordHolder::default(),
            picker: Picker::default(),
            removable_devices: RemovableDevices::default(),
            shortcut: Shortcut::new(start_dir).with_mount_points(mount_points),
            sudo_command: None,
            trash: Trash::new(binds)?,
            tui_applications: TuiApplications::new(TUIS_PATH),
            window: ContentWindow::new(0, 80),
        })
    }

    pub fn reset(&mut self) {
        self.input.reset();
        self.completion.reset();
        self.bulk.reset();
    }

    /// Fill the input string with the currently selected completion.
    pub fn input_complete(&mut self, tab: &mut Tab) -> Result<()> {
        self.fill_completion(tab)?;
        self.window.reset(self.completion.len());
        Ok(())
    }

    fn fill_completion(&mut self, tab: &mut Tab) -> Result<()> {
        match tab.edit_mode {
            Edit::InputCompleted(InputCompleted::Cd) => self.completion.cd(
                &self.input.string(),
                &tab.directory.path.as_os_str().to_string_lossy(),
            ),
            Edit::InputCompleted(InputCompleted::Exec) => {
                self.completion.exec(&self.input.string())
            }
            Edit::InputCompleted(InputCompleted::Search) => {
                let files = match tab.display_mode {
                    Display::Preview => vec![],
                    Display::Tree => tab.search.complete(tab.tree.displayable().content()),
                    // Display::Flagged => tab.search.complete(self.flagged.content()),
                    Display::Directory => tab.search.complete(tab.directory.content()),
                };
                self.completion.search(files);
                Ok(())
            }
            Edit::InputCompleted(InputCompleted::Action) => {
                self.completion.command(&self.input.string())
            }
            _ => Ok(()),
        }
    }

    pub fn find_encrypted_drive_mount_point(&self) -> Option<std::path::PathBuf> {
        let device = self.encrypted_devices.selected()?;
        if !device.is_mounted() {
            return None;
        }
        let mount_point = device.mount_point()?;
        Some(std::path::PathBuf::from(mount_point))
    }

    pub fn find_removable_mount_point(&mut self) -> Option<std::path::PathBuf> {
        let Some(device) = &self.removable_devices.selected() else {
            return None;
        };
        if !device.is_mounted() {
            return None;
        }
        Some(std::path::PathBuf::from(&device.path))
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    pub fn mount_remote(&mut self, current_path: &str) {
        let input = self.input.string();
        if let Some(remote_builder) = Remote::from_input(input) {
            remote_builder.mount(current_path);
        }
        self.input.reset();
    }

    /// Remove a flag file from Jump mode
    pub fn remove_selected_flagged(&mut self) -> Result<()> {
        self.flagged.remove_selected();
        Ok(())
    }

    pub fn trash_delete_permanently(&mut self) -> Result<()> {
        self.trash.delete_permanently()
    }

    /// Move the selected flagged file to the trash.
    pub fn trash_single_flagged(&mut self) -> Result<()> {
        let filepath = self
            .flagged
            .selected()
            .context("no flagged file")?
            .to_owned();
        self.flagged.remove_selected();
        self.trash.trash(&filepath)?;
        Ok(())
    }

    /// Delete the selected flagged file.
    pub fn delete_single_flagged(&mut self) -> Result<()> {
        let filepath = self
            .flagged
            .selected()
            .context("no flagged file")?
            .to_owned();
        self.flagged.remove_selected();
        if filepath.is_dir() {
            std::fs::remove_dir_all(filepath)?;
        } else {
            std::fs::remove_file(filepath)?;
        }
        Ok(())
    }

    pub fn delete_flagged_files(&mut self) -> Result<()> {
        let nb = self.flagged.len();
        for pathbuf in self.flagged.content.iter() {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(pathbuf)?;
            } else {
                std::fs::remove_file(pathbuf)?;
            }
        }
        self.flagged.clear();
        log_line!("Deleted {nb} flagged files");
        Ok(())
    }

    pub fn clear_sudo_attributes(&mut self) -> Result<()> {
        self.password_holder.reset();
        drop_sudo_privileges()?;
        self.sudo_command = None;
        Ok(())
    }

    /// Insert a char in the input string.
    pub fn input_insert(&mut self, char: char) -> Result<()> {
        self.input.insert(char);
        Ok(())
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh_shortcuts(
        &mut self,
        mount_points: &[&std::path::Path],
        left_path: &std::path::Path,
        right_path: &std::path::Path,
    ) {
        self.shortcut.refresh(mount_points, left_path, right_path)
    }

    pub fn shortcut_from_char(&mut self, c: char) -> bool {
        let Some(index) = index_from_a(c) else {
            return false;
        };
        if index < self.shortcut.len() {
            self.shortcut.set_index(index);
            self.window.scroll_to(index);
            return true;
        }
        false
    }

    pub fn context_from_char(&mut self, c: char) -> bool {
        let Some(index) = index_from_a(c) else {
            return false;
        };
        if index < self.context.len() {
            self.context.set_index(index);
            self.window.scroll_to(index);
            return true;
        }
        false
    }

    pub fn completion_reset(&mut self) {
        self.completion.reset();
    }

    pub fn completion_tab(&mut self) {
        self.input.replace(self.completion.current_proposition())
    }

    pub fn len(&self, edit_mode: Edit) -> usize {
        match edit_mode {
            Edit::Navigate(navigate) => self.apply_method(navigate, |variant| variant.len()),
            Edit::InputCompleted(_) => self.completion.len(),
            _ => 0,
        }
    }

    pub fn index(&self, edit_mode: Edit) -> usize {
        match edit_mode {
            Edit::Navigate(navigate) => self.apply_method(navigate, |variant| variant.index()),
            Edit::InputCompleted(_) => self.completion.index,
            _ => 0,
        }
    }

    pub fn page_down(&mut self, navigate: Navigate) {
        for _ in 0..10 {
            self.next(navigate)
        }
    }

    pub fn page_up(&mut self, navigate: Navigate) {
        for _ in 0..10 {
            self.prev(navigate)
        }
    }

    pub fn completion_prev(&mut self, input_completed: InputCompleted) {
        self.completion.prev();
        self.window
            .scroll_to(self.index(Edit::InputCompleted(input_completed)));
    }

    pub fn completion_next(&mut self, input_completed: InputCompleted) {
        self.completion.next();
        self.window
            .scroll_to(self.index(Edit::InputCompleted(input_completed)));
    }

    pub fn next(&mut self, navigate: Navigate) {
        self.apply_method_mut(navigate, |variant| variant.next());
        self.window.scroll_to(self.index(Edit::Navigate(navigate)));
    }

    pub fn prev(&mut self, navigate: Navigate) {
        self.apply_method_mut(navigate, |variant| variant.prev());
        self.window.scroll_to(self.index(Edit::Navigate(navigate)));
    }

    pub fn set_index(&mut self, index: usize, navigate: Navigate) {
        self.apply_method_mut(navigate, |variant| variant.set_index(index));
        self.window.scroll_to(self.index(Edit::Navigate(navigate)))
    }

    fn apply_method_mut<F, T>(&mut self, navigate: Navigate, func: F) -> T
    where
        F: FnOnce(&mut dyn Selectable) -> T,
    {
        match navigate {
            Navigate::CliApplication => func(&mut self.cli_applications),
            Navigate::Compress => func(&mut self.compression),
            Navigate::Context => func(&mut self.context),
            Navigate::EncryptedDrive => func(&mut self.encrypted_devices),
            Navigate::History => func(&mut self.history),
            Navigate::Marks(_) => func(&mut self.marks),
            Navigate::RemovableDevices => func(&mut self.removable_devices),
            Navigate::Shortcut => func(&mut self.shortcut),
            Navigate::Trash => func(&mut self.trash),
            Navigate::TuiApplication => func(&mut self.tui_applications),
            Navigate::Cloud => func(&mut self.cloud),
            Navigate::Picker => func(&mut self.picker),
            Navigate::Flagged => func(&mut self.flagged),
        }
    }

    fn apply_method<F, T>(&self, navigate: Navigate, func: F) -> T
    where
        F: FnOnce(&dyn Selectable) -> T,
    {
        match navigate {
            Navigate::CliApplication => func(&self.cli_applications),
            Navigate::Compress => func(&self.compression),
            Navigate::Context => func(&self.context),
            Navigate::EncryptedDrive => func(&self.encrypted_devices),
            Navigate::History => func(&self.history),
            Navigate::Marks(_) => func(&self.marks),
            Navigate::RemovableDevices => func(&self.removable_devices),
            Navigate::Shortcut => func(&self.shortcut),
            Navigate::Trash => func(&self.trash),
            Navigate::TuiApplication => func(&self.tui_applications),
            Navigate::Cloud => func(&self.cloud),
            Navigate::Picker => func(&self.picker),
            Navigate::Flagged => func(&self.flagged),
        }
    }

    /// Draw a navigation menu with its simple `draw_menu` method.
    ///
    /// # Errors
    ///
    /// Some mode can't be displayed directly and this method will raise an error.
    /// It's the responsability of the caller to check beforehand.
    pub fn draw_navigate(&self, canvas: &mut dyn Canvas, navigate: Navigate) -> Result<()> {
        match navigate {
            Navigate::Compress => self.compression.draw_menu(canvas, &self.window),
            Navigate::Shortcut => self.shortcut.draw_menu(canvas, &self.window),
            Navigate::Marks(_) => self.marks.draw_menu(canvas, &self.window),
            Navigate::TuiApplication => self.tui_applications.draw_menu(canvas, &self.window),
            Navigate::CliApplication => self.cli_applications.draw_menu(canvas, &self.window),
            Navigate::EncryptedDrive => self.encrypted_devices.draw_menu(canvas, &self.window),
            Navigate::RemovableDevices => self.removable_devices.draw_menu(canvas, &self.window),
            _ => bail!("{navigate} requires more information to be displayed."),
        }
    }
}
