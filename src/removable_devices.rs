use anyhow::{anyhow, Result};

use crate::{
    constant_strings_paths::GIO,
    impl_selectable_content,
    utils::{current_uid, is_dir_empty, is_program_in_path},
};

/// Holds info about removable devices.
/// We can navigate this struct.
/// It requires a special method, oustide of `SelectableContent` trait,
/// allowing the mutate the inner element.
/// It can create itself from a `gio` command output.
#[derive(Debug, Clone, Default)]
pub struct RemovableDevices {
    pub content: Vec<Removable>,
    pub index: usize,
}

impl RemovableDevices {
    /// Creates itself from a `gio mount -l` command output.
    ///
    /// Output lines are filtered, looking for `activation_root`.
    /// Then we create a `Removable` instance for every line.
    /// If no line match or if any error happens, we return `None`.
    pub fn from_gio() -> Option<Self> {
        if !is_program_in_path(GIO) {
            return None;
        }
        let Ok(output) = std::process::Command::new(GIO)
            .args(["mount", "-li"])
            .output()
        else {
            return None;
        };
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return None;
        };

        let content: Vec<_> = stdout
            .lines()
            .filter(|line| line.contains("activation_root"))
            .map(Removable::from_gio)
            .filter_map(|removable| removable.ok())
            .collect();

        if content.is_empty() {
            None
        } else {
            Some(Self { content, index: 0 })
        }
    }

    /// Mutable reference to the selected element.
    /// None if the content is empty (aka no removable device detected)
    pub fn selected_mut(&mut self) -> Option<&mut Removable> {
        if self.content.is_empty() {
            None
        } else {
            Some(&mut self.content[self.index])
        }
    }
}

/// Holds a MTP device name, a path and a flag set to true
/// if the device is mounted.
#[derive(Debug, Clone, Default)]
pub struct Removable {
    pub device_name: String,
    pub path: String,
    pub is_mounted: bool,
}

impl Removable {
    /// Creates a `Removable` instance from a filtered `gio` command output.
    ///
    /// `gio mount -l`  will return a lot of information about mount points,
    /// including MTP (aka Android) devices.
    /// We don't check if the device actually exists, we just create the instance.
    fn from_gio(line: &str) -> Result<Self> {
        let device_name = line
            .replace("activation_root=mtp://", "")
            .replace('/', "")
            .trim()
            .to_owned();
        let uid = current_uid()?;
        let path = format!("/run/user/{uid}/gvfs/mtp:host={device_name}");
        let pb_path = std::path::Path::new(&path);
        let is_mounted = pb_path.exists() && !is_dir_empty(pb_path)?;
        log::info!("gio {device_name} - is_mounted {is_mounted}");
        Ok(Self {
            device_name,
            path,
            is_mounted,
        })
    }

    /// Mount a non mounted removable device.
    /// `Err` if the device is already mounted.
    /// Runs a `gio mount $device_name` command and check
    /// the result.
    /// The `is_mounted` flag is updated accordingly to the result.
    pub fn mount(&mut self) -> Result<()> {
        if self.is_mounted {
            return Err(anyhow!("Already mounted {name}", name = self.device_name));
        }
        self.is_mounted = std::process::Command::new(GIO)
            .args(vec!["mount", &self.format_for_gio()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?
            .wait()?
            .success();
        log::info!(
            target: "special",
            "Mounted {device}. Success ? {success}",
            device=self.device_name, success=self.is_mounted
        );
        Ok(())
    }

    /// Unount a mounted removable device.
    /// `Err` if the device isnt mounted.
    /// Runs a `gio mount $device_name` command and check
    /// the result.
    /// The `is_mounted` flag is updated accordingly to the result.
    pub fn umount(&mut self) -> Result<()> {
        if !self.is_mounted {
            return Err(anyhow!("Not mounted {name}", name = self.device_name));
        }
        self.is_mounted = !std::process::Command::new(GIO)
            .args(vec!["mount", &self.format_for_gio(), "-u"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?
            .wait()?
            .success();
        log::info!(
            target: "special",
            "Unmounted {device}. Success ? {success}",
            device=self.device_name,
            success=!self.is_mounted
        );
        Ok(())
    }

    /// Format itself as a valid `gio mount $device` argument.
    fn format_for_gio(&self) -> String {
        format!("mtp://{name}", name = self.device_name)
    }
}

impl_selectable_content!(Removable, RemovableDevices);
