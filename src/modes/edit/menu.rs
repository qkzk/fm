use anyhow::Context;
use anyhow::Result;

use crate::app::Tab;
use crate::common::is_program_in_path;
use crate::common::SSHFS_EXECUTABLE;
use crate::common::TUIS_PATH;
use crate::io::drop_sudo_privileges;
use crate::io::execute_and_capture_output_with_path;
use crate::log_info;
use crate::log_line;
use crate::modes::Bulk;
use crate::modes::CliApplications;
use crate::modes::Completion;
use crate::modes::Compresser;
use crate::modes::ContextMenu;
use crate::modes::CryptoDeviceOpener;
use crate::modes::Edit;
use crate::modes::Flagged;
use crate::modes::Input;
use crate::modes::InputCompleted;
use crate::modes::IsoDevice;
use crate::modes::Marks;
use crate::modes::MountCommands;
use crate::modes::Navigate;
use crate::modes::PasswordHolder;
use crate::modes::RemovableDevices;
use crate::modes::SelectableContent;
use crate::modes::Shortcut;
use crate::modes::Trash;
use crate::modes::TuiApplications;

pub struct Menu {
    /// Bulk rename
    pub bulk: Option<Bulk>,
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
    pub removable_devices: Option<RemovableDevices>,
    /// Predefined shortcuts
    pub shortcut: Shortcut,
    /// TUI application
    pub tui_applications: TuiApplications,
    /// The trash
    pub trash: Trash,
    /// Last sudo command ran
    pub sudo_command: Option<String>,
}

impl Menu {
    pub fn new(start_dir: &std::path::Path, mount_points: &[&std::path::Path]) -> Result<Self> {
        Ok(Self {
            sudo_command: None,
            compression: Compresser::default(),
            context: ContextMenu::default(),
            cli_applications: CliApplications::default(),
            tui_applications: TuiApplications::new(TUIS_PATH),
            removable_devices: None,
            password_holder: PasswordHolder::default(),
            bulk: None,
            iso_device: None,
            encrypted_devices: CryptoDeviceOpener::default(),
            trash: Trash::new()?,
            marks: Marks::read_from_config_file(),
            flagged: Flagged::default(),
            input: Input::default(),
            completion: Completion::default(),
            shortcut: Shortcut::new(start_dir).with_mount_points(mount_points),
        })
    }

    pub fn reset(&mut self) {
        self.input.reset();
        self.completion.reset();
        self.bulk = None;
    }

    /// Creats a new bulk instance if needed
    pub fn init_bulk(&mut self) {
        if self.bulk.is_none() {
            self.bulk = Some(Bulk::default());
        }
    }

    pub fn bulk_prev(&mut self) {
        self.init_bulk();
        if let Some(bulk) = &mut self.bulk {
            bulk.prev();
        }
    }

    pub fn bulk_next(&mut self) {
        self.init_bulk();
        if let Some(bulk) = &mut self.bulk {
            bulk.next();
        }
    }

    /// Set the index of bulk, if those are set.
    /// Does nothing if `self.bulk` is still None.
    pub fn bulk_set_index(&mut self, index: usize) {
        if let Some(bulk) = &mut self.bulk {
            bulk.set_index(index)
        }
    }

    /// Fill the input string with the currently selected completion.
    pub fn input_complete(&mut self, c: char, tab: &Tab) -> Result<()> {
        self.input.insert(c);
        self.fill_completion(tab)
    }

    fn fill_completion(&mut self, tab: &Tab) -> Result<()> {
        match tab.edit_mode {
            Edit::InputCompleted(InputCompleted::Goto) => self.completion.goto(
                &self.input.string(),
                &tab.directory.path.as_os_str().to_string_lossy(),
            ),
            Edit::InputCompleted(InputCompleted::Exec) => {
                self.completion.exec(&self.input.string())
            }
            Edit::InputCompleted(InputCompleted::Search) => {
                self.completion.search(tab.filenames(&self.input.string()));
                Ok(())
            }
            Edit::InputCompleted(InputCompleted::Command) => {
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
        let Some(devices) = &self.removable_devices else {
            return None;
        };
        let Some(device) = devices.selected() else {
            return None;
        };
        if !device.is_mounted() {
            return None;
        }
        Some(std::path::PathBuf::from(&device.path))
    }

    pub fn mount_removable(&mut self) -> Result<()> {
        let Some(devices) = &mut self.removable_devices else {
            return Ok(());
        };
        let Some(device) = devices.selected_mut() else {
            return Ok(());
        };
        if device.is_mounted() {
            return Ok(());
        }
        device.mount_simple()?;
        Ok(())
    }

    pub fn umount_removable(&mut self) -> Result<()> {
        let Some(devices) = &mut self.removable_devices else {
            return Ok(());
        };
        let Some(device) = devices.selected_mut() else {
            return Ok(());
        };
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
    pub fn refresh_shortcuts(&mut self, mount_points: &[&std::path::Path]) {
        self.shortcut.refresh(mount_points)
    }

    /// Set the index of removable devices, if those are set.
    /// Does nothing if `self.removable_devices` is still None.
    pub fn removable_set_index(&mut self, index: usize) {
        if let Some(removable) = &mut self.removable_devices {
            removable.set_index(index)
        }
    }

    /// Select the next element of the menu
    pub fn next(&mut self, navigate: Navigate) {
        match navigate {
            Navigate::Jump => self.flagged.next(),
            Navigate::Trash => self.trash.next(),
            Navigate::Shortcut => self.shortcut.next(),
            Navigate::Marks(_) => self.marks.next(),
            Navigate::Compress => self.compression.next(),
            Navigate::Context => self.context.next(),
            Navigate::BulkMenu => self.bulk_next(),
            Navigate::TuiApplication => self.tui_applications.next(),
            Navigate::CliApplication => self.cli_applications.next(),
            Navigate::EncryptedDrive => self.encrypted_devices.next(),
            Navigate::RemovableDevices => {
                if let Some(removable) = &mut self.removable_devices {
                    removable.next()
                }
            }
            _ => (),
        }
    }

    /// Select the previous element of the menu
    pub fn prev(&mut self, navigate: Navigate) {
        match navigate {
            Navigate::Jump => self.flagged.prev(),
            Navigate::Trash => self.trash.prev(),
            Navigate::Shortcut => self.shortcut.prev(),
            Navigate::Marks(_) => self.marks.prev(),
            Navigate::Compress => self.compression.prev(),
            Navigate::Context => self.context.prev(),
            Navigate::BulkMenu => self.bulk_prev(),
            Navigate::TuiApplication => self.tui_applications.prev(),
            Navigate::CliApplication => self.cli_applications.prev(),
            Navigate::EncryptedDrive => self.encrypted_devices.prev(),
            Navigate::RemovableDevices => {
                if let Some(removable) = &mut self.removable_devices {
                    removable.prev()
                }
            }
            _ => (),
        }
    }
}
