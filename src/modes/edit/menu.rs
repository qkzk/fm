use anyhow::Result;

use crate::common::TUIS_PATH;
use crate::modes::Bulk;
use crate::modes::CliApplications;
use crate::modes::Compresser;
use crate::modes::CryptoDeviceOpener;
use crate::modes::IsoDevice;
use crate::modes::Marks;
use crate::modes::MountCommands;
use crate::modes::PasswordHolder;
use crate::modes::RemovableDevices;
use crate::modes::SelectableContent;
use crate::modes::Trash;
use crate::modes::TuiApplications;

pub struct Menu {
    /// Hold password between their typing and usage
    pub password_holder: PasswordHolder,
    /// Last sudo command ran
    pub sudo_command: Option<String>,
    /// Compression methods
    pub compression: Compresser,
    /// MTP devices
    pub removable_devices: Option<RemovableDevices>,
    /// CLI applications
    pub cli_applications: CliApplications,
    /// TUI application
    pub tui_applications: TuiApplications,
    /// Bulk rename
    pub bulk: Option<Bulk>,
    /// Iso mounter. Set to None by default, dropped ASAP
    pub iso_device: Option<IsoDevice>,
    /// Encrypted devices opener
    pub encrypted_devices: CryptoDeviceOpener,
    /// The trash
    pub trash: Trash,
    /// Marks allows you to jump to a save mark
    pub marks: Marks,
}

impl Menu {
    pub fn new() -> Result<Self> {
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
        })
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

    pub fn trash_delete_permanently(&mut self) -> Result<()> {
        self.trash.delete_permanently()
    }
}
