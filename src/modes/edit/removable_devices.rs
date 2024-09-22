use std::fmt::Display;
use std::io::Read;

use anyhow::{anyhow, Result};

use crate::common::{current_uid, filename_from_path, is_dir_empty, is_in_path, MKDIR, MOUNT};
use crate::common::{EJECT_EXECUTABLE, GIO};
use crate::impl_content;
use crate::impl_selectable;
use crate::io::{
    drop_sudo_privileges, execute_and_output, execute_sudo_command, reset_sudo_faillock,
    set_sudo_session,
};
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
    pub fn find() -> Option<Self> {
        let mut content = Self::mtp_from_gio();
        content.extend(Self::usb_from_builder());

        if content.is_empty() {
            None
        } else {
            Some(Self { content, index: 0 })
        }
    }

    fn mtp_from_gio() -> Vec<Removable> {
        if !is_in_path(GIO) {
            return vec![];
        }
        let Ok(output) = execute_and_output(GIO, [MOUNT, "-li"]) else {
            return vec![];
        };
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return vec![];
        };

        stdout
            .lines()
            .filter(|line| line.contains("activation_root"))
            .map(Removable::from_gio)
            .filter_map(std::result::Result::ok)
            .collect()
    }

    fn usb_from_builder() -> Vec<Removable> {
        UsbDevicesBuilder::list_usb_disks().unwrap_or_else(|_| vec![])
    }
}

#[derive(Debug, Clone, Default)]
pub enum RemovableKind {
    #[default]
    Mtp,
    Usb,
}

impl Display for RemovableKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let kind = match self {
            Self::Mtp => "MTP",
            Self::Usb => "USB",
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
    pub is_ejected: bool,
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
        let kind = RemovableKind::Mtp;
        let is_ejected = false;
        log_info!("gio {name} - is_mounted {is_mounted}");
        Ok(Self {
            name,
            kind,
            path,
            is_mounted,
            is_ejected,
        })
    }

    fn from_usb(volume: String, mount_point: Option<String>, is_ejected: bool) -> Self {
        let (is_mounted, path) = mount_point
            .map(|p| (std::path::Path::new(&p).exists(), p))
            .unwrap_or_else(|| {
                let mp =
                    UsbDevicesBuilder::build_mount_path(&volume).unwrap_or_else(|| "".to_owned());
                (false, mp)
            });

        let name = UsbDevicesBuilder::get_usb_device_name(&volume)
            .map(|desc_name| format!("{} {}", volume, desc_name))
            .unwrap_or(volume);

        Self {
            name,
            kind: RemovableKind::Usb,
            is_mounted,
            path,
            is_ejected,
        }
    }

    /// Format itself as a valid `gio mount $device` argument.
    fn format_for_gio(&self) -> String {
        match self.kind {
            RemovableKind::Usb => {
                let fields = self.name.split(' ').collect::<Vec<&str>>();
                let volume = fields[0];
                log_info!("volume path: {volume}");
                volume.to_owned()
            }
            RemovableKind::Mtp => {
                format!("mtp://{name}", name = self.name)
            }
        }
    }

    /// Mount the devices
    pub fn mount_simple(&mut self, password_holder: &mut PasswordHolder) -> Result<bool> {
        self.mount("", password_holder)
    }

    fn eject(&self, password: &mut PasswordHolder) -> Result<bool> {
        let success = set_sudo_session(password)?;
        if !success {
            password.reset();
            return Ok(false);
        }
        let (success, stdout, stderr) = execute_sudo_command(&[EJECT_EXECUTABLE, &self.path])?;
        log_info!("eject: success: {success}, stdout {stdout}, stderr {stderr}");
        if !success {
            reset_sudo_faillock()?
        }
        password.reset();
        drop_sudo_privileges()?;
        Ok(success)
    }

    /// unmount the device. Eject the usb devices.
    pub fn umount_simple(&mut self, password_holder: &mut PasswordHolder) -> Result<bool> {
        let Ok(umount) = self.umount("", password_holder) else {
            return Ok(false);
        };
        Ok(umount)
    }

    /// True iff the device is an usb disk.
    pub fn is_usb(&self) -> bool {
        matches!(self.kind, RemovableKind::Usb)
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
    fn mount(&mut self, _: &str, password: &mut PasswordHolder) -> Result<bool> {
        if self.is_mounted {
            return Err(anyhow!("Already mounted {name}", name = self.name));
        }
        self.is_mounted = match self.kind {
            RemovableKind::Mtp => execute_and_output(GIO, ["mount", &self.format_for_gio()])?
                .status
                .success(),
            RemovableKind::Usb => {
                let success = set_sudo_session(password)?;
                if !success {
                    password.reset();
                    return Ok(false);
                }
                let (success, stdout, stderr) =
                    execute_sudo_command(&[MKDIR, "-p", self.path.as_str()])?;
                log_info!("mkdir: success {success} -- stdout {stdout} -- stderr {stderr}");
                let (success, stdout, stderr) = execute_sudo_command(&[
                    MOUNT,
                    self.format_for_gio().as_str(),
                    self.path.as_str(),
                ])?;
                password.reset();
                log_info!("mount: success {success} -- stdout {stdout} -- stderr {stderr}");

                if !success {
                    reset_sudo_faillock()?
                }
                password.reset();
                drop_sudo_privileges()?;
                success
            }
        };

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
    fn umount(&mut self, _: &str, password: &mut PasswordHolder) -> Result<bool> {
        if !self.is_mounted {
            return Err(anyhow!("Not mounted {name}", name = self.name));
        }
        self.is_mounted = match self.kind {
            RemovableKind::Mtp => execute_and_output(GIO, ["mount", &self.format_for_gio(), "-u"])?
                .status
                .success(),
            RemovableKind::Usb => self.eject(password)?,
        };

        log_info!(
            "Unmounted {device}. Success ? {success}",
            device = self.name,
            success = self.is_mounted
        );
        Ok(!self.is_mounted)
    }
}

impl MountRepr for Removable {
    /// String representation of the device
    fn as_string(&self) -> Result<String> {
        Ok(format!(
            " {kind}- {name} ",
            kind = self.kind,
            name = self.name.clone()
        ))
    }

    fn device_name(&self) -> Result<String> {
        self.as_string()
    }
}

struct UsbDevicesBuilder {}

impl UsbDevicesBuilder {
    fn list_usb_disks() -> Result<Vec<Removable>> {
        let proc_mounts = Self::read_proc_mounts()?;

        let usb_disks = std::fs::read_dir("/sys/block")?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|device_path| device_path.is_dir())
            .filter_map(|device_path| {
                if filename_from_path(&device_path).ok()?.starts_with("sd")
                    && Self::is_removable(&device_path.join("removable")).ok()?
                {
                    Some(Self::list_volumes(&device_path))
                } else {
                    None
                }
            })
            .flatten()
            .flatten()
            .map(|volume| {
                let is_ejected = Self::is_device_ejected(std::path::Path::new(&volume));

                Removable::from_usb(
                    volume.to_owned(),
                    Self::get_mount_point_for_volume(&proc_mounts, &volume),
                    is_ejected,
                )
            })
            .collect();
        Ok(usb_disks)
    }

    fn read_proc_mounts() -> Result<String> {
        let mut file = std::fs::File::open("/proc/mounts")?;
        let mut proc_mounts = String::new();
        file.read_to_string(&mut proc_mounts)?;
        Ok(proc_mounts)
    }

    /// Lists volumes for a given device path (like /dev/sdd).
    /// Check partitions in /sys/class/block/{device_name}/{device_name}{partition_number}
    fn list_volumes(device: &std::path::Path) -> Result<Vec<String>> {
        let device_name = filename_from_path(device)?;

        let volumes = std::fs::read_dir(format!("/sys/class/block/{}/", device_name))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|p| p.join("partition").exists())
            .map(|p| filename_from_path(&p).unwrap_or_default().to_owned())
            .map(|partition_name| format!("/dev/{}", partition_name))
            .collect();

        Ok(volumes)
    }

    /// Extract the mount point of a volume
    fn get_mount_point_for_volume(proc_mount: &str, volume: &str) -> Option<String> {
        for line in proc_mount.lines().filter(|line| line.starts_with(volume)) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 2 {
                return Some(fields[1].to_owned());
            }
        }
        None
    }

    /// Checks if a device has been ejected.
    fn is_device_ejected(device: &std::path::Path) -> bool {
        !device.exists()
    }

    /// Checks if a device is removable
    fn is_removable(p: &std::path::Path) -> Result<bool> {
        // Open the file
        let mut file = std::fs::File::open(p)?;

        // Read the contents of the file
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        // Check if the content is exactly "1\n"
        Ok(content.starts_with('1'))
    }

    /// Gets the user-friendly name of a USB device from its volume path.
    fn get_usb_device_name(volume: &str) -> Option<String> {
        // Extract the base device name (e.g., /dev/sdd1 -> sdd)
        let volume_path = std::path::Path::new(volume);
        let device_name = volume_path
            .file_name()?
            .to_str()?
            .trim_end_matches(char::is_numeric);

        // Path to the sysfs directory for the device
        let device_sysfs_path = std::path::Path::new("/sys/class/block")
            .join(device_name)
            .join("device");

        // Read the manufacturer, product, and serial files
        let vendor = std::fs::read_to_string(device_sysfs_path.join("vendor"))
            .ok()
            .unwrap_or_default();
        let model = std::fs::read_to_string(device_sysfs_path.join("model"))
            .ok()
            .unwrap_or_default();

        // Combine the information into a single string
        let user_friendly_name = format!("{} {}", vendor.trim(), model.trim(),);

        if user_friendly_name.trim().is_empty() {
            None
        } else {
            Some(user_friendly_name)
        }
    }

    /// Gets the current logged-in username.
    fn get_current_username() -> Option<String> {
        std::env::var("USER").ok()
    }

    /// Gets the UUID of the filesystem on the given volume.
    fn get_volume_uuid(volume: &str) -> Option<String> {
        let output = std::process::Command::new("lsblk")
            .arg("-lfo")
            .arg("UUID")
            .arg(volume)
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        log_line!("get_volume_uuid. stdout: {stdout} - stderr: {stderr}");

        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout);
            let uuid = content
                .split_whitespace()
                .collect::<Vec<&str>>()
                .last()
                .map(|s| s.to_owned().to_owned());
            uuid
        } else {
            None
        }
    }

    /// Constructs the mount path in /run/media/{user}/{uuid} format for the given volume path.
    fn build_mount_path(volume: &str) -> Option<String> {
        let username = Self::get_current_username()?;
        let uuid = Self::get_volume_uuid(volume)?;
        Some(format!("/run/media/{}/{}", username, uuid))
    }
}

impl_selectable!(RemovableDevices);
impl_content!(Removable, RemovableDevices);
