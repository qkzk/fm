use std::fmt::Display;

use anyhow::{anyhow, Result};

use crate::common::{current_uid, is_dir_empty, is_program_in_path};
use crate::common::{EJECT_EXECUTABLE, GIO};
use crate::impl_content;
use crate::impl_selectable;
use crate::io::{execute, execute_and_output};
use crate::log_info;
use crate::log_line;
use crate::modes::PasswordHolder;
use crate::modes::{MountCommands, MountRepr};

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
        let Ok(output) = execute_and_output(GIO, ["mount", "-li"]) else {
            return None;
        };
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return None;
        };

        let mut content: Vec<_> = stdout
            .lines()
            .filter(|line| line.contains("activation_root"))
            .map(Removable::from_gio)
            .filter_map(std::result::Result::ok)
            .collect();

        content.extend(Self::find_usb(stdout).into_iter());

        if content.is_empty() {
            None
        } else {
            Some(Self { content, index: 0 })
        }
    }

    fn find_usb(stdout: String) -> Vec<Removable> {
        // TODO rewrite completely, ugly solution
        let mut found_can_eject = false;
        let max_dist = 10;
        let mut line_counter = 0;
        let mut devices = vec![];
        let mut name = "";
        let mut is_mounted = false;
        for line in stdout.lines() {
            if !found_can_eject && line.contains("Drive(") {
                let elems: Vec<&str> = line.split(':').collect();
                name = elems[1];
            }
            if line.contains("can_eject=1") {
                found_can_eject = true;
                continue;
            }
            if found_can_eject && line.contains("Mount(") {
                is_mounted = true;
                if let Ok(device) = Removable::usb_from_gio(line, name, is_mounted) {
                    devices.push(device);
                }

                found_can_eject = false;
                line_counter = 0;
                is_mounted = false;
            }
            if found_can_eject {
                line_counter += 1;
            }
            if line_counter >= max_dist {
                found_can_eject = false;
                line_counter = 0;
                is_mounted = false;
            }
        }
        devices
    }
}

#[derive(Debug, Clone, Default)]
pub enum RemovableKind {
    #[default]
    MTP,
    USB,
}

impl Display for RemovableKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let kind = match self {
            Self::MTP => "MTP",
            Self::USB => "USB",
        };
        writeln!(f, "{kind}",)
    }
}

/// Holds a MTP device name, a path and a flag set to true
/// if the device is mounted.
#[derive(Debug, Clone, Default)]
pub struct Removable {
    pub name: String,
    pub kind: RemovableKind,
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
        let name = line
            .replace("activation_root=mtp://", "")
            .replace('/', "")
            .trim()
            .to_owned();
        let uid = current_uid()?;
        let path = format!("/run/user/{uid}/gvfs/mtp:host={name}");
        let pb_path = std::path::Path::new(&path);
        let is_mounted = pb_path.exists() && !is_dir_empty(pb_path)?;
        let kind = RemovableKind::MTP;
        log_info!("gio {name} - is_mounted {is_mounted}");
        Ok(Self {
            name,
            kind,
            path,
            is_mounted,
        })
    }

    fn usb_from_gio(line: &str, name: &str, is_mounted: bool) -> Result<Self> {
        // "    Mount(0): 134 MB Volume -> file:///run/media/quentin/cfb31e6a-f288-4415-9900-a4822c736fd2"
        let kind = RemovableKind::USB;
        let path = line.split("file://").collect::<Vec<&str>>()[1].to_owned();
        let name = name.trim().to_string();

        Ok(Self {
            name,
            kind,
            path,
            is_mounted,
        })
    }

    /// Format itself as a valid `gio mount $device` argument.
    fn format_for_gio(&self) -> String {
        format!("mtp://{name}", name = self.name)
    }

    pub fn mount_simple(&mut self) -> Result<bool> {
        self.mount("", &mut PasswordHolder::default())
    }

    fn eject(&self) -> Result<()> {
        execute(EJECT_EXECUTABLE, &[&self.path])?;
        Ok(())
    }

    pub fn umount_simple(&mut self) -> Result<bool> {
        let Ok(umount) = self.umount("", &mut PasswordHolder::default()) else {
            return Ok(false);
        };
        if !umount {
            return Ok(false);
        }
        match self.kind {
            RemovableKind::MTP => (),
            RemovableKind::USB => self.eject()?,
        }
        return Ok(true);
    }
}

impl MountCommands for Removable {
    /// True if the device is mounted
    fn is_mounted(&self) -> bool {
        self.is_mounted
    }

    /// Mount a non mounted removable device.
    /// `Err` if the device is already mounted.
    /// Runs a `gio mount $name` command and check
    /// the result.
    /// The `is_mounted` flag is updated accordingly to the result.
    fn mount(&mut self, _: &str, _: &mut PasswordHolder) -> Result<bool> {
        if self.is_mounted {
            return Err(anyhow!("Already mounted {name}", name = self.name));
        }
        self.is_mounted = execute_and_output(GIO, ["mount", &self.format_for_gio()])?
            .status
            .success();
        log_line!(
            "Mounted {device}. Success ? {success}",
            device = self.name,
            success = self.is_mounted
        );
        Ok(self.is_mounted)
    }

    /// Unount a mounted removable device.
    /// `Err` if the device isnt mounted.
    /// Runs a `gio mount $device_name` command and check
    /// the result.
    /// The `is_mounted` flag is updated accordingly to the result.
    fn umount(&mut self, _: &str, _: &mut PasswordHolder) -> Result<bool> {
        if !self.is_mounted {
            return Err(anyhow!("Not mounted {name}", name = self.name));
        }
        self.is_mounted = execute_and_output(GIO, ["mount", &self.format_for_gio(), "-u"])?
            .status
            .success();
        log_line!(
            "Unmounted {device}. Success ? {success}",
            device = self.name,
            success = !self.is_mounted
        );
        Ok(!self.is_mounted)
    }
}

impl MountRepr for Removable {
    /// String representation of the device
    fn as_string(&self) -> Result<String> {
        Ok(self.name.clone())
    }

    fn device_name(&self) -> Result<String> {
        self.as_string()
    }
}

// impl_selectable_content!(Removable, RemovableDevices);
impl_selectable!(RemovableDevices);
impl_content!(Removable, RemovableDevices);
