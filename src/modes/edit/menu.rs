use anyhow::Context;
use anyhow::Result;

use crate::common::TUIS_PATH;
use crate::io::drop_sudo_privileges;
use crate::log_line;
use crate::modes::Bulk;
use crate::modes::CliApplications;
use crate::modes::Completion;
use crate::modes::Compresser;
use crate::modes::CryptoDeviceOpener;
use crate::modes::Flagged;
use crate::modes::Input;
use crate::modes::IsoDevice;
use crate::modes::Marks;
use crate::modes::MountCommands;
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
            shortcut: Shortcut::new(&start_dir).with_mount_points(mount_points),
        })
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
}
