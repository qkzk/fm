use std::fmt::Display;
use std::io::Read;

use anyhow::{anyhow, Result};

use crate::common::{current_uid, is_dir_empty, is_program_in_path};
use crate::common::{EJECT_EXECUTABLE, GIO};
use crate::impl_content;
use crate::impl_selectable;
use crate::io::{
    execute, execute_and_output, execute_sudo_command, execute_sudo_command_with_password,
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

        content.extend(Self::find_usb().into_iter());

        if content.is_empty() {
            None
        } else {
            Some(Self { content, index: 0 })
        }
    }

    fn find_usb() -> Vec<Removable> {
        list_usb_disks().unwrap_or_else(|_| vec![])
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
        let kind = RemovableKind::MTP;
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
                let mp = build_mount_path(&volume).unwrap_or_else(|| "".to_owned());
                (false, mp)
            });

        let name = get_usb_device_name(&volume)
            .map(|desc_name| format!("{} {}", volume, desc_name))
            .unwrap_or(volume);

        Self {
            name,
            kind: RemovableKind::USB,
            is_mounted,
            path,
            is_ejected,
        }
    }

    /// Format itself as a valid `gio mount $device` argument.
    fn format_for_gio(&self) -> String {
        match self.kind {
            RemovableKind::USB => {
                let fields = self.name.split(' ').collect::<Vec<&str>>();
                let volume = fields[0];
                log_info!("volume path: {volume}");
                volume.to_owned()
            }
            RemovableKind::MTP => {
                format!("mtp://{name}", name = self.name)
            }
        }
    }

    pub fn mount_simple(&mut self) -> Result<bool> {
        self.mount("", &mut PasswordHolder::default())
    }

    fn eject(&self) -> Result<()> {
        execute(EJECT_EXECUTABLE, &[&self.path])?;
        Ok(())
    }

    pub fn umount_simple(&mut self) -> Result<bool> {
        match self.kind {
            RemovableKind::MTP => {
                let Ok(umount) = self.umount("", &mut PasswordHolder::default()) else {
                    return Ok(false);
                };
                if !umount {
                    return Ok(false);
                }
            }
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
        self.is_mounted = match self.kind {
            RemovableKind::MTP => execute_and_output(GIO, ["mount", &self.format_for_gio(), "-u"])?
                .status
                .success(),
            RemovableKind::USB => {
                execute_sudo_command(&["eject", &self.format_for_gio()])?;
                true
            }
        };
        if matches!(self.kind, RemovableKind::USB) {}
        log_info!(
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

fn list_usb_disks() -> Result<Vec<Removable>> {
    let mut usb_disks = Vec::new();
    // Path to the directory containing block devices
    let sys_path = std::path::Path::new("/sys/block");

    // Iterate over entries in the block device directory
    for entry in std::fs::read_dir(sys_path)? {
        let entry = entry?;
        let path = entry.path();

        let Ok(mut file) = std::fs::File::open("/proc/mounts") else {
            return Ok(vec![]);
        };

        let mut proc_mounts = String::new();
        file.read_to_string(&mut proc_mounts)?;

        // Check if the entry is a directory
        if path.is_dir() {
            let device_name = path.file_name().unwrap().to_str().unwrap();
            // Check if the device name starts with "sd" (common for USB disks)
            if device_name.starts_with("sd") {
                // Check if the device has a "removable" flag file
                let removable_path = path.join("removable");
                if is_removable(&removable_path)? {
                    log_info!("USB -- Found {p} which is removable", p = path.display());
                    // Likely a USB disk, get volume information
                    let volumes = list_volumes(&path);
                    log_info!("USB -- Volumes of {p}: {volumes:?}", p = path.display());
                    let vols: Vec<Removable> = volumes
                        .iter()
                        .map(|volume| {
                            let is_ejected = is_device_ejected(std::path::Path::new(volume));

                            Removable::from_usb(
                                volume.to_owned(),
                                get_mount_point_for_volume(&proc_mounts, volume),
                                is_ejected,
                            )
                        })
                        .collect();
                    usb_disks.extend(vols);
                }
            }
        }
    }

    Ok(usb_disks)
}

/// Lists volumes for a given device path (like /dev/sdd).
fn list_volumes(device: &std::path::Path) -> Vec<String> {
    let mut volumes = Vec::new();
    let device_name = device.file_name().unwrap().to_str().unwrap();

    // Check partitions in /sys/class/block/{device_name}/{device_name}{partition_number}
    let sys_block_path = format!("/sys/class/block/{}/", device_name);
    if let Ok(entries) = std::fs::read_dir(&sys_block_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let partition_path = entry.path();
                if partition_path.join("partition").exists() {
                    if let Some(partition_name) =
                        partition_path.file_name().and_then(|n| n.to_str())
                    {
                        let partition = format!("/dev/{}", partition_name);
                        volumes.push(partition);
                    }
                }
            }
        }
    }

    volumes
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
    Ok(content.starts_with("1"))
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
    let output = std::process::Command::new("blkid")
        .arg("-s")
        .arg("UUID")
        .arg("-o")
        .arg("value")
        .arg(volume)
        .output()
        .ok()?;

    if output.status.success() {
        let uuid = String::from_utf8_lossy(&output.stdout);
        Some(uuid.trim().to_string())
    } else {
        None
    }
}

/// Constructs the mount path in /run/media/{user}/{uuid} format for the given volume path.
fn build_mount_path(volume: &str) -> Option<String> {
    let username = get_current_username()?;
    let uuid = get_volume_uuid(volume)?;
    Some(format!("/run/media/{}/{}", username, uuid))
}

impl_selectable!(RemovableDevices);
impl_content!(Removable, RemovableDevices);
