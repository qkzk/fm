use std::{borrow::Cow, path::PathBuf};

use anyhow::Result;
use serde::Deserialize;
use sysinfo::Disks;

use crate::common::{current_username, MKDIR};
use crate::io::{
    drop_sudo_privileges, execute_and_output, execute_sudo_command, reset_sudo_faillock,
    set_sudo_session, CowStr, DrawMenu,
};
use crate::modes::{get_devices_json, MountCommands, MountParameters, MountRepr, PasswordHolder};
use crate::{impl_content, impl_selectable, log_info};

// TODO: copied from cryptsetup, should be unified someway
#[derive(Default, Deserialize, Debug)]
pub struct BlockDevice {
    fstype: Option<String>,
    pub path: String,
    uuid: Option<String>,
    fsver: Option<String>,
    mountpoint: Option<String>,
    name: Option<String>,
    label: Option<String>,
    #[serde(default)]
    children: Vec<BlockDevice>,
    #[serde(default)]
    is_encrypted_device: bool,
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
        if self.is_encrypted_device {
            return true;
        }
        let Some(fstype) = &self.fstype else {
            return false;
        };
        fstype.contains("crypto")
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

    fn format_mount_parameters(&mut self, _username: &str) -> Vec<String> {
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
        // sudo -k
        execute_sudo_command(&["-k".to_owned()])?;
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
        let is_mounted = if self.is_mounted() { "M" } else { "U" };
        let is_cypto = if self.is_crypto() { "C" } else { " " };
        let label = if let Some(label) = &self.label {
            label
        } else {
            ""
        };
        Ok(if let Some(mountpoint) = &self.mountpoint {
            format!(
                "{is_mounted}{is_cypto} {} {label} -> {}",
                self.path, mountpoint
            )
        } else {
            format!("{is_mounted}{is_cypto} {} {label}", self.path)
        })
    }

    fn device_name(&self) -> Result<String> {
        self.as_string()
    }
}

#[derive(Debug)]
pub enum Mountable {
    Remote((String, String)),
    Device(BlockDevice),
}

impl Mountable {
    fn as_string(&self) -> Result<String> {
        match &self {
            Self::Device(device) => device.as_string(),
            Self::Remote((remote_desc, local_path)) => {
                Ok(format!("MS {remote_desc} -> {local_path}"))
            }
        }
    }

    pub fn is_crypto(&self) -> bool {
        match &self {
            Self::Device(device) => device.is_crypto(),
            Self::Remote(_) => false,
        }
    }

    pub fn is_mounted(&self) -> bool {
        match &self {
            Self::Device(device) => device.is_mounted(),
            Self::Remote(_) => true,
        }
    }

    pub fn path(&self) -> &str {
        match &self {
            Self::Device(device) => device.path.as_str(),
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
        let value: serde_json::Value = serde_json::from_str(&json_content)?;

        let blockdevices_value = value
            .get("blockdevices")
            .ok_or("Missing 'blockdevices' field in JSON")?;

        let devices: Vec<BlockDevice> = serde_json::from_value(blockdevices_value.clone())?;
        let mut content: Vec<Mountable> = vec![];
        for top_level in devices.into_iter() {
            if !top_level.children.is_empty() {
                let is_crypto = top_level.is_crypto();
                for mut children in top_level.children.into_iter() {
                    children.is_encrypted_device = is_crypto;
                    content.push(Mountable::Device(children));
                }
            } else if top_level.uuid.is_some() {
                content.push(Mountable::Device(top_level))
            }
        }
        Ok(content)
    }

    pub fn umount_selected_no_password(&mut self) -> Result<bool> {
        match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount_no_password(),
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
            Mountable::Remote((_name, mountpoint)) => mountpoint,
        };
        Some(PathBuf::from(mountpoint))
    }

    pub fn umount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<()> {
        let username = current_username()?;
        let success = match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount(&username, password_holder)?,
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

impl_selectable!(Mount);
impl_content!(Mount, Mountable);
impl DrawMenu<Mountable> for Mount {}
