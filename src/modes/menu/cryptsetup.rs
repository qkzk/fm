use std::borrow::Cow;

use anyhow::{anyhow, Context, Result};
use sysinfo::Disks;

use crate::common::{current_username, is_in_path, CRYPTSETUP, LSBLK, MKDIR};
use crate::io::{
    drop_sudo_privileges, execute_and_output, execute_sudo_command,
    execute_sudo_command_with_password, reset_sudo_faillock, set_sudo_session, CowStr, DrawMenu,
};
use crate::modes::{MountCommands, MountParameters, MountRepr, PasswordHolder, PasswordKind};
use crate::{impl_content, impl_selectable, log_info};

/// Possible actions on encrypted drives
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockDeviceAction {
    MOUNT,
    UMOUNT,
}

/// get devices list from lsblk
/// Return the output of
/// ```bash
/// lsblk -l -o FSTYPE,PATH,UUID,FSVER,MOUNTPOINT,PARTLABEL
/// ```
/// as a String.
fn get_devices() -> Result<String> {
    let output = execute_and_output(LSBLK, ["-l", "-o", "FSTYPE,PATH,UUID,FSVER,MOUNTPOINT"])?;
    Ok(String::from_utf8(output.stdout)?)
}

/// True iff `lsblk` and `cryptsetup` are in path.
/// Nothing here can be done without those programs.
pub fn lsblk_and_cryptsetup_installed() -> bool {
    is_in_path(LSBLK) && is_in_path(CRYPTSETUP)
}

/// Represent an encrypted device.
/// Those attributes comes from cryptsetup.
#[derive(Debug, Default, Clone)]
pub struct CryptoDevice {
    fs_type: String,
    path: String,
    uuid: String,
    fs_ver: String,
    mountpoints: Option<String>,
    device_name: Option<String>,
}

impl CryptoDevice {
    /// Parse the output of a lsblk formated line into a struct
    fn from_line(line: &str) -> Result<Self> {
        let mut crypo_device = Self::default();
        crypo_device.update_from_line(line)?;
        Ok(crypo_device)
    }

    fn update_from_line(&mut self, line: &str) -> Result<()> {
        let strings = line.split_whitespace();
        let mut params: Vec<Option<String>> = vec![None; 5];
        for (count, param) in strings.enumerate() {
            params[count] = Some(param.to_owned());
        }
        self.fs_type = params
            .remove(0)
            .context("CryptoDevice: parameter shouldn't be None")?;
        self.path = params
            .remove(0)
            .context("CryptoDevice: parameter shouldn't be None")?;
        self.uuid = params
            .remove(0)
            .context("CryptoDevice: parameter shouldn't be None")?;
        self.fs_ver = params
            .remove(0)
            .context("CryptoDevice: parameter shouldn't be None")?;
        self.mountpoints = params.remove(0);
        Ok(())
    }

    fn device_name(&self) -> String {
        self.device_name
            .clone()
            .unwrap_or_else(|| self.uuid.clone())
    }

    fn format_luksopen_parameters(&self) -> [String; 4] {
        [
            CRYPTSETUP.to_owned(),
            "open".to_owned(),
            self.path.clone(),
            self.uuid.clone(),
        ]
    }
    fn format_luksclose_parameters(&self) -> [String; 3] {
        [
            CRYPTSETUP.to_owned(),
            "luksClose".to_owned(),
            self.device_name(),
        ]
    }

    pub fn mount_point(&self) -> Option<String> {
        let disks = Disks::new_with_refreshed_list();
        disks
            .iter()
            .map(|d| d.mount_point())
            .filter_map(|p| p.to_str())
            .map(|s| s.to_owned())
            .find(|s| s.contains(&self.uuid))
    }

    fn set_device_name(&mut self) -> Result<()> {
        let output = execute_and_output(LSBLK, ["-l", "-n", self.path.as_str()])?;
        log_info!(
            "is opened ? output of lsblk\nstdout: {}\nstderr{}",
            String::from_utf8(output.stdout.clone())?,
            String::from_utf8(output.stderr)?
        );
        let output = String::from_utf8(output.stdout)?;
        if let Some(s) = output.lines().nth(1) {
            self.device_name = Some(
                s.split_whitespace()
                    .next()
                    .context("mapped point: shouldn't be empty")?
                    .to_owned(),
            );
        } else {
            self.device_name = None;
        }
        Ok(())
    }

    fn open_mount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        let root_path = std::path::Path::new("/");
        self.set_device_name()?;
        if self.is_mounted() {
            Err(anyhow!("luks open mount: device is already mounted"))
        } else {
            // sudo
            let success = set_sudo_session(password)?;
            if !success {
                password.reset();
                return Ok(false);
            }
            // open
            let (success, stdout, stderr) = execute_sudo_command_with_password(
                &self.format_luksopen_parameters(),
                password
                    .cryptsetup()
                    .as_ref()
                    .context("cryptsetup password isn't set")?,
                root_path,
            )?;
            password.reset();
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
            if !success {
                return Ok(false);
            }
            self.mount(username, password)
        }
    }

    fn umount_close(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        if !self.umount(username, password)? {
            return Ok(false);
        }
        // close
        let (success, stdout, stderr) = execute_sudo_command(&self.format_luksclose_parameters())?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }
}

impl MountParameters for CryptoDevice {
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            MKDIR.to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.device_name()),
        ]
    }

    fn format_mount_parameters(&mut self, username: &str) -> Vec<String> {
        vec![
            "mount".to_owned(),
            format!("/dev/mapper/{}", self.uuid),
            format!("/run/media/{}/{}", username, self.device_name()),
        ]
    }

    fn format_umount_parameters(&self, username: &str) -> Vec<String> {
        vec![
            "umount".to_owned(),
            format!("/run/media/{}/{}", username, self.device_name()),
        ]
    }
}

impl MountCommands for CryptoDevice {
    /// True if there's a mount point for this drive.
    /// It's only valid if we mounted the device since it requires
    /// the uuid to be in the mount point.
    fn is_mounted(&self) -> bool {
        self.mount_point().is_some()
    }

    fn mount(&mut self, username: &str, _: &mut PasswordHolder) -> Result<bool> {
        self.set_device_name()?;
        // mkdir
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_mkdir_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        if !success {
            return Ok(false);
        }
        // mount
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_mount_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        if !success {
            return Ok(false);
        }
        // sudo -k
        execute_sudo_command(&["-k".to_owned()])?;
        Ok(success)
    }

    fn umount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        self.set_device_name()?;
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

impl MountRepr for CryptoDevice {
    /// String representation of the device.
    fn as_string(&self) -> Result<String> {
        Ok(if let Some(mount_point) = self.mount_point() {
            format!("{} -> {}", self.path, mount_point)
        } else {
            format!("{} - not mounted", self.path)
        })
    }

    fn device_name(&self) -> Result<String> {
        self.as_string()
    }
}

/// Holds a list of devices and an index.
/// It's a navigable content so the index follows the selection
/// of the user.
#[derive(Debug, Clone, Default)]
pub struct CryptoDeviceOpener {
    pub content: Vec<CryptoDevice>,
    index: usize,
}

impl CryptoDeviceOpener {
    /// Updates itself from the output of cryptsetup.
    pub fn update(&mut self) -> Result<()> {
        self.content = get_devices()?
            .lines()
            .filter(|line| line.contains("crypto"))
            .map(CryptoDevice::from_line)
            .filter_map(|r| r.ok())
            .collect();
        self.index = 0;
        Ok(())
    }

    /// Set a password for the selected device.
    pub fn set_password(
        &mut self,
        password_kind: PasswordKind,
        password: String,
        password_holder: &mut PasswordHolder,
    ) {
        match password_kind {
            PasswordKind::SUDO => password_holder.set_sudo(password),
            PasswordKind::CRYPTSETUP => password_holder.set_cryptsetup(password),
        }
    }

    /// Open and mount the selected device.
    pub fn mount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<bool> {
        let username = current_username()?;
        let success = self.content[self.index].open_mount(&username, password_holder)?;
        if !success {
            reset_sudo_faillock()?
        }
        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(success)
    }

    /// Unmount and close the selected device.
    pub fn umount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<()> {
        let username = current_username()?;
        let success = self.content[self.index].umount_close(&username, password_holder)?;
        if !success {
            reset_sudo_faillock()?
        }
        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(())
    }
}

impl_selectable!(CryptoDeviceOpener);
impl_content!(CryptoDevice, CryptoDeviceOpener);

impl CowStr for CryptoDevice {
    fn cow_str(&self) -> Cow<str> {
        self.as_string().unwrap_or_default().into()
    }
}

impl DrawMenu<CryptoDevice> for CryptoDeviceOpener {}
