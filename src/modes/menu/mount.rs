use std::{borrow::Cow, path::PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use sysinfo::Disks;

use crate::common::{current_username, is_in_path, CRYPTSETUP, LSBLK, MKDIR, UDISKSCTL};
use crate::io::{
    drop_sudo_privileges, execute_and_output, execute_sudo_command,
    execute_sudo_command_with_password, reset_sudo_faillock, set_sudo_session, CowStr, DrawMenu,
};
use crate::modes::{MountCommands, MountParameters, MountRepr, PasswordHolder};
use crate::{impl_content, impl_selectable, log_info};

/// Possible actions on encrypted drives
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockDeviceAction {
    MOUNT,
    UMOUNT,
}

#[derive(Debug)]
pub struct EncryptedBlockDevice {
    pub path: String,
    uuid: Option<String>,
    mountpoint: Option<String>,
    label: Option<String>,
    parent: Option<String>,
}

impl MountParameters for EncryptedBlockDevice {
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            MKDIR.to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.uuid.clone().unwrap()),
        ]
    }

    fn format_mount_parameters(&self, username: &str) -> Vec<String> {
        vec![
            "mount".to_owned(),
            format!("/dev/mapper/{}", self.uuid.clone().unwrap()),
            format!("/run/media/{}/{}", username, self.uuid.clone().unwrap()),
        ]
    }

    fn format_umount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            "udisksctl".to_owned(),
            "unmount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }
}

impl MountRepr for EncryptedBlockDevice {
    /// String representation of the device.
    fn as_string(&self) -> Result<String> {
        let mut repr = format!(
            "{is_mounted}C {path} {label}",
            is_mounted = if self.is_mounted() { "M" } else { "U" },
            label = self.label_repr(),
            path = self.path
        );
        if let Some(mountpoint) = &self.mountpoint {
            repr.push_str(" -> ");
            repr.push_str(mountpoint)
        }
        Ok(repr)
    }
}

impl From<BlockDevice> for EncryptedBlockDevice {
    fn from(device: BlockDevice) -> Self {
        EncryptedBlockDevice {
            path: device.path,
            uuid: device.uuid,
            mountpoint: device.mountpoint,
            label: device.label,
            parent: None,
        }
    }
}
impl EncryptedBlockDevice {
    fn set_parent(&mut self, parent_uuid: &Option<String>) {
        self.parent = parent_uuid.clone()
    }

    pub fn open_mount_crypto(
        &self,
        username: &str,
        password_holder: &mut PasswordHolder,
    ) -> Result<bool> {
        let success = is_in_path(CRYPTSETUP)
            && self.set_sudo_session(password_holder)?
            && self.execute_luks_open(password_holder)?
            && self.execute_mkdir_crypto(username)?
            && self.execute_mount_crypto(username)?;
        drop_sudo_privileges()?;
        Ok(success)
    }

    fn set_sudo_session(&self, password_holder: &mut PasswordHolder) -> Result<bool> {
        if !set_sudo_session(password_holder)? {
            password_holder.reset();
            return Ok(false);
        }
        Ok(true)
    }

    fn execute_luks_open(&self, password_holder: &mut PasswordHolder) -> Result<bool> {
        let (success, stdout, stderr) = execute_sudo_command_with_password(
            &self.format_luksopen_parameters(),
            password_holder
                .cryptsetup()
                .as_ref()
                .context("cryptsetup password_holder isn't set")?,
            std::path::Path::new("/"),
        )?;
        password_holder.reset();
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    fn execute_mkdir_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_mkdir_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    fn execute_mount_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_mount_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    pub fn umount_close_crypto(
        &self,
        username: &str,
        password_holder: &mut PasswordHolder,
    ) -> Result<bool> {
        let success = is_in_path(CRYPTSETUP)
            && self.set_sudo_session(password_holder)?
            && self.execute_umount_crypto(username)?
            && self.execute_luks_close()?;
        drop_sudo_privileges()?;
        Ok(success)
    }

    fn execute_umount_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_umount_parameters(username))?;
        if !success {
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        }
        Ok(success)
    }

    fn execute_luks_close(&self) -> Result<bool> {
        let (success, stdout, stderr) = execute_sudo_command(&self.format_luksclose_parameters())?;
        if !success {
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        }
        Ok(success)
    }

    fn format_luksopen_parameters(&self) -> [String; 4] {
        [
            CRYPTSETUP.to_owned(),
            "open".to_owned(),
            self.path.clone(),
            self.uuid.clone().unwrap(),
        ]
    }

    fn format_luksclose_parameters(&self) -> [String; 3] {
        [
            CRYPTSETUP.to_owned(),
            "close".to_owned(),
            self.parent.clone().unwrap(),
        ]
    }

    const fn is_crypto(&self) -> bool {
        true
    }

    fn label_repr(&self) -> &str {
        if let Some(label) = &self.label {
            label
        } else {
            ""
        }
    }

    /// True if there's a mount point for this drive.
    /// It's only valid if we mounted the device since it requires
    /// the uuid to be in the mount point.
    fn is_mounted(&self) -> bool {
        self.mountpoint.is_some()
    }
}

#[derive(Default, Deserialize, Debug)]
pub struct BlockDevice {
    fstype: Option<String>,
    pub path: String,
    uuid: Option<String>,
    mountpoint: Option<String>,
    name: Option<String>,
    label: Option<String>,
    hotplug: bool,
    #[serde(default)]
    children: Vec<BlockDevice>,
}

impl BlockDevice {
    fn device_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| self.uuid.as_ref().unwrap().clone())
    }

    fn mount_no_password(&mut self) -> Result<bool> {
        let mut args = self.format_mount_parameters("");
        let output = execute_and_output(&args.remove(0), &args)?;
        if output.status.success() {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn umount_no_password(&mut self) -> Result<bool> {
        let mut args = self.format_umount_parameters("");
        let output = execute_and_output(&args.remove(0), &args)?;
        if output.status.success() {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn is_crypto(&self) -> bool {
        let Some(fstype) = &self.fstype else {
            return false;
        };
        fstype.contains("crypto")
    }

    fn is_loop(&self) -> bool {
        self.path.contains("loop")
    }

    fn prefix_repr(&self) -> &str {
        match (self.is_loop(), self.hotplug) {
            (true, _) => "L",
            (false, true) => "R",
            _ => " ",
        }
    }

    fn label_repr(&self) -> &str {
        if let Some(label) = &self.label {
            label
        } else {
            ""
        }
    }
}

impl MountParameters for BlockDevice {
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            MKDIR.to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.device_name()),
        ]
    }

    fn format_mount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            "udisksctl".to_owned(),
            "mount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }

    fn format_umount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            "udisksctl".to_owned(),
            "unmount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }
}

impl MountCommands for BlockDevice {
    /// True if there's a mount point for this drive.
    /// It's only valid if we mounted the device since it requires
    /// the uuid to be in the mount point.
    fn is_mounted(&self) -> bool {
        self.mountpoint.is_some()
    }

    fn mount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        // sudo
        let success = set_sudo_session(password)?;
        password.reset();
        if !success {
            return Ok(false);
        }
        // mount
        let args_sudo = self.format_mount_parameters(username);
        let (success, stdout, stderr) = execute_sudo_command(&args_sudo)?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        if !success {
            return Ok(false);
        }
        drop_sudo_privileges()?;
        Ok(success)
    }

    fn umount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        // sudo
        let success = set_sudo_session(password)?;
        password.reset();
        if !success {
            return Ok(false);
        }
        // unmount
        let (_, stdout, stderr) = execute_sudo_command(&self.format_umount_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);

        Ok(true)
    }
}

impl MountRepr for BlockDevice {
    /// String representation of the device.
    fn as_string(&self) -> Result<String> {
        let mut repr = format!(
            "{is_mounted}{prefix} {path} {label}",
            is_mounted = if self.is_mounted() { "M" } else { "U" },
            prefix = self.prefix_repr(),
            label = self.label_repr(),
            path = self.path
        );
        if let Some(mountpoint) = &self.mountpoint {
            repr.push_str(" -> ");
            repr.push_str(mountpoint)
        }
        Ok(repr)
    }
}

#[derive(Debug)]
pub enum Mountable {
    Remote((String, String)),
    Encrypted(EncryptedBlockDevice),
    Device(BlockDevice),
}

impl Mountable {
    fn as_string(&self) -> Result<String> {
        match &self {
            Self::Device(device) => device.as_string(),
            Self::Encrypted(device) => device.as_string(),
            Self::Remote((remote_desc, local_path)) => {
                Ok(format!("MS {remote_desc} -> {local_path}"))
            }
        }
    }

    pub fn is_crypto(&self) -> bool {
        match &self {
            Self::Device(device) => device.is_crypto(),
            Self::Encrypted(device) => device.is_crypto(),
            Self::Remote(_) => false,
        }
    }

    pub fn is_mounted(&self) -> bool {
        match &self {
            Self::Device(device) => device.is_mounted(),
            Self::Encrypted(device) => device.is_mounted(),
            Self::Remote(_) => true,
        }
    }

    pub fn path(&self) -> &str {
        match &self {
            Self::Device(device) => device.path.as_str(),
            Self::Encrypted(device) => device.path.as_str(),
            Self::Remote((_, local_path)) => local_path.as_str(),
        }
    }
}

impl CowStr for Mountable {
    fn cow_str(&self) -> Cow<str> {
        self.as_string().unwrap_or_default().into()
    }
}

#[derive(Default, Debug)]
pub struct Mount {
    pub content: Vec<Mountable>,
    index: usize,
}

impl Mount {
    pub fn update(&mut self, disks: &Disks) -> Result<()> {
        self.index = 0;

        self.content = Self::build_from_json()?;
        self.extend_with_remote(disks);

        log_info!("{self:#?}");
        Ok(())
    }

    fn build_from_json() -> Result<Vec<Mountable>> {
        let json_content = get_devices_json()?;
        match Self::from_json(json_content) {
            Ok(content) => Ok(content),
            Err(e) => {
                log_info!("update error {e:#?}");
                Ok(vec![])
            }
        }
    }

    fn extend_with_remote(&mut self, disks: &Disks) {
        self.content.extend(
            disks
                .iter()
                .filter(|d| d.file_system().to_string_lossy().contains("sshfs"))
                .map(|d| {
                    Mountable::Remote((
                        d.name().to_string_lossy().to_string(),
                        d.mount_point().to_string_lossy().to_string(),
                    ))
                })
                .collect::<Vec<_>>(),
        );
    }

    fn from_json(json_content: String) -> Result<Vec<Mountable>, Box<dyn std::error::Error>> {
        let devices: Vec<BlockDevice> = Self::read_blocks_from_json(json_content)?;
        let mut content = vec![];
        for top_level in devices.into_iter() {
            let is_crypto = top_level.is_crypto();
            if !top_level.children.is_empty() {
                Self::push_children(is_crypto, &mut content, top_level);
            } else if top_level.uuid.is_some() {
                Self::push_parent(is_crypto, &mut content, top_level)
            }
        }
        Ok(content)
    }

    fn read_blocks_from_json(
        json_content: String,
    ) -> Result<Vec<BlockDevice>, Box<dyn std::error::Error>> {
        let mut value: serde_json::Value = serde_json::from_str(&json_content)?;

        let blockdevices_value: serde_json::Value = value
            .get_mut("blockdevices")
            .ok_or("Missing 'blockdevices' field in JSON")?
            .take();
        Ok(serde_json::from_value(blockdevices_value)?)
    }

    fn push_children(is_crypto: bool, content: &mut Vec<Mountable>, top_level: BlockDevice) {
        for children in top_level.children.into_iter() {
            if is_crypto {
                let mut encrypted_children: EncryptedBlockDevice = children.into();
                encrypted_children.set_parent(&top_level.uuid);
                content.push(Mountable::Encrypted(encrypted_children));
            } else {
                content.push(Mountable::Device(children));
            }
        }
    }

    fn push_parent(is_crypto: bool, content: &mut Vec<Mountable>, top_level: BlockDevice) {
        if is_crypto {
            content.push(Mountable::Encrypted(top_level.into()))
        } else {
            content.push(Mountable::Device(top_level))
        }
    }

    pub fn umount_selected_no_password(&mut self) -> Result<bool> {
        match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount_no_password(),
            Mountable::Encrypted(_device) => {
                unreachable!("Encrypted devices can't be unmounted without password.")
            }
            Mountable::Remote((_name, mountpoint)) => {
                let output = execute_and_output("umount", [mountpoint.as_str()])?;
                let success = output.status.success();
                log_info!(
                    "umount {mountpoint}:\nstdout: {stdout}\nstderr: {stderr}",
                    stdout = String::from_utf8(output.stdout)?,
                    stderr = String::from_utf8(output.stderr)?,
                );
                Ok(success)
            }
        }
    }

    pub fn mount_selected_no_password(&mut self) -> Result<bool> {
        match &mut self.content[self.index] {
            Mountable::Device(device) => device.mount_no_password(),
            Mountable::Encrypted(_device) => {
                unreachable!("Encrypted devices can't be mounted without password.")
            }
            Mountable::Remote(_) => Ok(false),
        }
    }

    /// Open and mount the selected device.
    pub fn mount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<bool> {
        let success = match &mut self.content[self.index] {
            Mountable::Device(device) => {
                let username = current_username()?;
                let success = device.mount(&username, password_holder)?;
                if !success {
                    reset_sudo_faillock()?
                }
                success
            }
            Mountable::Encrypted(_device) => {
                unreachable!("EncryptedBlockDevice should impl its own method")
            }
            Mountable::Remote(_) => false,
        };

        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(success)
    }

    pub fn selected_mount_point(&self) -> Option<PathBuf> {
        let selected = self.selected()?;
        let mountpoint: &str = match selected {
            Mountable::Device(device) => {
                let Some(mountpoint) = &device.mountpoint else {
                    return None;
                };
                mountpoint
            }
            Mountable::Encrypted(device) => {
                let Some(mountpoint) = &device.mountpoint else {
                    return None;
                };
                mountpoint
            }
            Mountable::Remote((_name, mountpoint)) => mountpoint,
        };
        Some(PathBuf::from(mountpoint))
    }

    pub fn umount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<()> {
        let username = current_username()?;
        let success = match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount(&username, password_holder)?,
            Mountable::Encrypted(_device) => {
                unreachable!("EncryptedBlockDevice should impl its own method")
            }
            Mountable::Remote((_, mountpoint)) => umount_remote(mountpoint, password_holder)?,
        };
        if !success {
            reset_sudo_faillock()?
        }
        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(())
    }
}

fn umount_remote(mountpoint: &str, password_holder: &mut PasswordHolder) -> Result<bool> {
    let success = set_sudo_session(password_holder)?;
    password_holder.reset();
    if !success {
        return Ok(false);
    }
    let (success, stdout, stderr) = execute_sudo_command(&["umount", mountpoint])?;
    if !success {
        log_info!(
            "umount remote failed:\nstdout: {}\nstderr: {}",
            stdout,
            stderr
        );
    }

    Ok(success)
}

/// True iff `lsblk` and `cryptsetup` are in path.
/// Nothing here can be done without those programs.
pub fn lsblk_and_udisksctl_installed() -> bool {
    is_in_path(LSBLK) && is_in_path(UDISKSCTL)
}

pub fn get_devices_json() -> Result<String> {
    let output = execute_and_output(
        LSBLK,
        [
            "--json",
            "-o",
            "FSTYPE,PATH,UUID,MOUNTPOINT,NAME,LABEL,HOTPLUG",
        ],
    )?;
    Ok(String::from_utf8(output.stdout)?)
}

impl_selectable!(Mount);
impl_content!(Mount, Mountable);
impl DrawMenu<Mountable> for Mount {}
