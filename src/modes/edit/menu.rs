use anyhow::Result;

use crate::common::TUIS_PATH;
use crate::modes::CliApplications;
use crate::modes::Compresser;
use crate::modes::MountCommands;
use crate::modes::PasswordHolder;
use crate::modes::RemovableDevices;
use crate::modes::SelectableContent;
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
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            sudo_command: None,
            compression: Compresser::default(),
            cli_applications: CliApplications::default(),
            tui_applications: TuiApplications::new(TUIS_PATH),
            removable_devices: None,
            password_holder: PasswordHolder::default(),
        }
    }
}

impl Menu {
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
}
