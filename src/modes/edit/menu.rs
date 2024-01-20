use anyhow::Context;
use anyhow::Result;

use crate::app::Tab;
use crate::common::is_program_in_path;
use crate::common::CLI_PATH;
use crate::common::MARKS_FILEPATH;
use crate::common::SSHFS_EXECUTABLE;
use crate::common::TUIS_PATH;
use crate::config::Bindings;
use crate::io::drop_sudo_privileges;
use crate::io::execute_and_capture_output_with_path;
use crate::log_info;
use crate::log_line;
use crate::modes::Bulk;
use crate::modes::CLApplications;
use crate::modes::CliApplications;
use crate::modes::Completion;
use crate::modes::Compresser;
use crate::modes::Content;
use crate::modes::ContentWindow;
use crate::modes::ContextMenu;
use crate::modes::CryptoDeviceOpener;
use crate::modes::Display;
use crate::modes::Edit;
use crate::modes::Flagged;
use crate::modes::History;
use crate::modes::Input;
use crate::modes::InputCompleted;
use crate::modes::IsoDevice;
use crate::modes::Marks;
use crate::modes::MountCommands;
use crate::modes::Navigate;
use crate::modes::PasswordHolder;
use crate::modes::RemovableDevices;
use crate::modes::Selectable;
use crate::modes::Shortcut;
use crate::modes::Trash;
use crate::modes::TuiApplications;

pub struct Menu {
    /// Window for scrollable menus
    pub window: ContentWindow,
    /// Bulk rename
    pub bulk: Bulk,
    /// CLI applications
    pub cli_applications: CliApplications,
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
    /// Iso mounter. Set to None by default, dropped ASAP
    pub iso_device: Option<IsoDevice>,
    /// Marks allows you to jump to a save mark
    pub marks: Marks,
    /// Hold password between their typing and usage
    pub password_holder: PasswordHolder,
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
    ) -> Result<Self> {
        Ok(Self {
            bulk: Bulk::default(),
            cli_applications: CliApplications::new(CLI_PATH).update_desc_size(),
            completion: Completion::default(),
            compression: Compresser::default(),
            context: ContextMenu::default(),
            encrypted_devices: CryptoDeviceOpener::default(),
            flagged: Flagged::new(vec![], 80),
            history: History::default(),
            input: Input::default(),
            iso_device: None,
            marks: Marks::new(MARKS_FILEPATH),
            password_holder: PasswordHolder::default(),
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
                    Display::Tree => tab.search.complete_tree(&tab.tree),
                    Display::Flagged => tab.search.complete_flagged(&self.flagged),
                    Display::Directory => tab.search.complete_directory(tab),
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
        let Some(device) = self.encrypted_devices.selected() else {
            return None;
        };
        if !device.is_mounted() {
            return None;
        }
        let Some(mount_point) = device.mount_point() else {
            return None;
        };
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

    pub fn mount_removable(&mut self) -> Result<()> {
        if self.removable_devices.is_empty() {
            return Ok(());
        };
        let device = &mut self.removable_devices.content[self.removable_devices.index];
        if device.is_mounted() {
            return Ok(());
        }
        device.mount_simple()?;
        Ok(())
    }

    pub fn umount_removable(&mut self) -> Result<()> {
        if self.removable_devices.is_empty() {
            return Ok(());
        };
        let device = &mut self.removable_devices.content[self.removable_devices.index];
        if !device.is_mounted() {
            return Ok(());
        }
        device.umount_simple()?;
        Ok(())
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    pub fn mount_remote(&mut self, current_path: &str) {
        let input = self.input.string();
        let user_hostname_remotepath: Vec<&str> = input.split(' ').collect();
        self.input.reset();

        if !is_program_in_path(SSHFS_EXECUTABLE) {
            log_info!("{SSHFS_EXECUTABLE} isn't in path");
            return;
        }
        if user_hostname_remotepath.len() != 3 {
            log_info!(
                "Wrong number of parameters for {SSHFS_EXECUTABLE}, expected 3, got {nb}",
                nb = user_hostname_remotepath.len()
            );
            return;
        };

        let (username, hostname, remote_path) = (
            user_hostname_remotepath[0],
            user_hostname_remotepath[1],
            user_hostname_remotepath[2],
        );
        let first_arg = &format!("{username}@{hostname}:{remote_path}");
        let output = execute_and_capture_output_with_path(
            SSHFS_EXECUTABLE,
            current_path,
            &[first_arg, current_path],
        );
        log_info!("{SSHFS_EXECUTABLE} {first_arg} output {output:?}");
        log_line!("{SSHFS_EXECUTABLE} {first_arg} output {output:?}");
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

    fn apply_method_mut<F, T>(&mut self, navigate: Navigate, func: F) -> T
    where
        F: FnOnce(&mut dyn Selectable) -> T,
    {
        match navigate {
            Navigate::BulkMenu => func(&mut self.bulk),
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
        }
    }

    fn apply_method<F, T>(&self, navigate: Navigate, func: F) -> T
    where
        F: FnOnce(&dyn Selectable) -> T,
    {
        match navigate {
            Navigate::BulkMenu => func(&self.bulk),
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
        }
    }
}
