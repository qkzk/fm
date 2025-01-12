use std::os::unix::process::ExitStatusExt;
use std::{borrow::Cow, path::PathBuf};

use anyhow::{Context, Result};

use crate::common::{current_username, LSBLK, MKDIR};
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
    path: String,
    uuid: Option<String>,
    fsver: Option<String>,
    mountpoint: Option<String>,
    name: Option<String>,
    #[serde(default)]
    children: Vec<BlockDevice>,
}

impl BlockDevice {
    fn device_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| self.uuid.as_ref().unwrap().clone())
    }

    pub fn mount_no_password(&mut self) -> Result<bool> {
        let mut args = self.format_mount_parameters("");
        let output = execute_and_output(&args.remove(0), &args)?;
        if output.status.success() {
            Ok(true)
        } else {
            Ok(false)
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
        Ok(if let Some(mountpoint) = &self.mountpoint {
            format!("{} -> {}", self.path, mountpoint)
        } else {
            format!("{} - not mounted", self.path)
        })
    }

    fn device_name(&self) -> Result<String> {
        self.as_string()
    }
}

#[derive(Default, Deserialize, Debug)]
pub struct Mount {
    #[serde(rename = "blockdevices")]
    pub content: Vec<BlockDevice>,
    #[serde(default)]
    index: usize,
}

impl Mount {
    pub fn update(&mut self) -> Result<()> {
        let json_content = get_devices_json()?;
        self.content = Self::from_json(json_content)?;
        self.index = 0;
        log_info!("{self:#?}");
        Ok(())
    }

    fn from_json(json_content: String) -> Result<Vec<BlockDevice>> {
        let root = serde_json::from_str::<Mount>(&json_content)?;
        let mut content: Vec<BlockDevice> = vec![];
        for top_level in root.content.into_iter() {
            if top_level.uuid.is_some() {
                content.push(top_level)
            } else if !top_level.children.is_empty() {
                for children in top_level.children.into_iter() {
                    content.push(children)
                }
            }
        }
        Ok(content)
    }

    pub fn mount_selected_no_password(&mut self) -> Result<bool> {
        self.content[self.index].mount_no_password()
    }

    /// Open and mount the selected device.
    pub fn mount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<bool> {
        let username = current_username()?;
        let success = self.content[self.index].mount(&username, password_holder)?;
        if !success {
            reset_sudo_faillock()?
        }
        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(success)
    }

    pub fn selected_mount_point(&self) -> Option<PathBuf> {
        let selected = self.selected()?;
        let Some(mountpoint) = &selected.mountpoint else {
            return None;
        };
        Some(PathBuf::from(mountpoint))
    }

    pub fn umount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<()> {
        let username = current_username()?;
        let success = self.content[self.index].umount(&username, password_holder)?;
        if !success {
            reset_sudo_faillock()?
        }
        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(())
    }
}

impl CowStr for BlockDevice {
    fn cow_str(&self) -> Cow<str> {
        self.as_string().unwrap_or_default().into()
    }
}

impl_selectable!(Mount);
impl_content!(Mount, BlockDevice);
impl DrawMenu<BlockDevice> for Mount {}

use serde::Deserialize;
