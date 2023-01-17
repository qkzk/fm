use std::io::Write;
use std::process::{Command, Stdio};

use log::info;
use sysinfo::{DiskExt, System, SystemExt};

use crate::fm_error::{FmError, FmResult};
use crate::impl_selectable_content;
use crate::utils::current_username;

#[derive(Debug, Clone, Copy)]
pub enum PasswordKind {
    SUDO,
    CRYPTSETUP,
}

#[derive(Debug, Clone, Copy)]
pub enum EncryptedAction {
    MOUNT,
    UMOUNT,
}

impl std::fmt::Display for PasswordKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let asker = match self {
            Self::SUDO => "sudo   ",
            Self::CRYPTSETUP => "device ",
        };
        write!(f, "{}", asker)
    }
}

#[derive(Default, Clone, Debug)]
pub struct PasswordHolder {
    sudo: Option<String>,
    cryptsetup: Option<String>,
}

impl PasswordHolder {
    /// Set the sudo password
    pub fn with_sudo(mut self, password: &str) -> Self {
        self.sudo = Some(password.to_owned());
        self
    }

    /// Set the cryptsetup password
    pub fn with_cryptsetup(mut self, passphrase: &str) -> Self {
        self.cryptsetup = Some(passphrase.to_owned());
        self
    }

    pub fn set_sudo(&mut self, password: String) {
        self.sudo = Some(password)
    }

    pub fn set_cryptsetup(&mut self, passphrase: String) {
        self.cryptsetup = Some(passphrase)
    }

    /// Reads the cryptsetup password
    pub fn cryptsetup(&self) -> FmResult<String> {
        Ok(self
            .cryptsetup
            .clone()
            .ok_or_else(|| FmError::custom("PasswordHolder", "cryptsetup password isn't set"))?)
    }

    /// Reads the sudo password
    pub fn sudo(&self) -> FmResult<String> {
        Ok(self
            .sudo
            .clone()
            .ok_or_else(|| FmError::custom("PasswordHolder", "sudo password isn't set"))?)
    }

    pub fn has_sudo(&self) -> bool {
        info!("has sudo ? {:?}", self);
        self.sudo.is_some()
    }

    pub fn has_cryptsetup(&self) -> bool {
        self.cryptsetup.is_some()
    }

    pub fn reset(&mut self) {
        self.sudo = None;
        self.cryptsetup = None;
    }
}

/// get devices list from lsblk
/// Return the output of
/// ```bash
/// lsblk -l -o FSTYPE,PATH,UUID,FSVER,MOUNTPOINT,PARTLABEL
/// ```
/// as a String.
pub fn get_devices() -> FmResult<String> {
    let output = Command::new("lsblk")
        .args(&vec!["-l", "-o", "FSTYPE,PATH,UUID,FSVER,MOUNTPOINT"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;
    Ok(String::from_utf8(output.stdout)?)
}

/// Parse the output of an lsblk detailed output and filter the line
/// Containing "crypto" aka Luks encrypted crypto devices
pub fn filter_crypto_devices_lines(output: String, key: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| line.contains(key))
        .map(|line| line.into())
        .collect()
}

/// run a sudo command requiring a password (generally to establish the password.)
/// Since I can't send 2 passwords at a time, it will only work with the sudo password
fn sudo_password(args: &[String], password: &str) -> FmResult<(bool, String, String)> {
    info!("sudo {:?}", args);
    let mut child = Command::new("sudo")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let child_stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| FmError::custom("run_privileged_command", "couldn't open child stdin"))?;
    child_stdin.write_all(&format!("{}\n", password).as_bytes())?;
    drop(child_stdin);

    let output = child.wait_with_output()?;
    Ok((
        output.status.success(),
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}

/// Run a passwordless sudo command.
/// Returns stdout & stderr
fn sudo(args: &[String]) -> FmResult<(bool, String, String)> {
    info!("sudo {:?}", args);
    let child = Command::new("sudo")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let output = child.wait_with_output()?;
    Ok((
        output.status.success(),
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}

#[derive(Debug, Default, Clone)]
pub struct CryptoDevice {
    fs_type: String,
    path: String,
    uuid: String,
    fs_ver: String,
    mountpoints: Option<String>,
}

impl CryptoDevice {
    /// Parse the output of a lsblk formated line into a struct
    pub fn from_line(line: &str) -> FmResult<Self> {
        let mut crypo_device = Self::default();
        crypo_device.update_from_line(line)?;
        Ok(crypo_device)
    }

    fn update_from_line(&mut self, line: &str) -> FmResult<()> {
        let mut strings = line.split_whitespace();
        let mut params: Vec<Option<String>> = vec![None; 5];
        let mut count = 0;
        while let Some(param) = strings.next() {
            params[count] = Some(param.to_owned());
            count += 1;
        }
        self.fs_type = params
            .remove(0)
            .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?;
        self.path = params
            .remove(0)
            .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?;
        self.uuid = params
            .remove(0)
            .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?;
        self.fs_ver = params
            .remove(0)
            .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?;
        self.mountpoints = params.remove(0);
        Ok(())
    }

    fn format_luksopen_parameters(&self) -> [String; 4] {
        [
            "cryptsetup".to_owned(),
            "open".to_owned(),
            self.path.clone(),
            self.uuid.clone(),
        ]
    }

    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            "mkdir".to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.uuid),
        ]
    }

    fn format_mount_parameters(&self, username: &str) -> [String; 3] {
        [
            "mount".to_owned(),
            format!("/dev/mapper/{}", self.uuid),
            format!("/run/media/{}/{}", username, self.uuid),
        ]
    }

    fn format_umount_parameters(&self, username: &str) -> [String; 2] {
        [
            "umount".to_owned(),
            format!("/run/media/{}/{}", username, self.uuid),
        ]
    }

    fn format_luksclose_parameters(&self) -> [String; 3] {
        [
            "cryptsetup".to_owned(),
            "luksClose".to_owned(),
            self.uuid.to_owned(),
        ]
    }

    pub fn open_mount(&mut self, username: &str, passwords: &mut PasswordHolder) -> FmResult<bool> {
        if self.mount_point().is_some() {
            Err(FmError::custom(
                "luks open mount",
                "device is already mounted",
            ))
        } else {
            // sudo
            let (success, _, _) = sudo_password(
                &["-S".to_owned(), "ls".to_owned(), "/root".to_owned()],
                &passwords.sudo()?,
            )?;
            if !success {
                return Ok(false);
            }
            // open
            let (success, stdout, stderr) =
                sudo_password(&self.format_luksopen_parameters(), &passwords.cryptsetup()?)?;
            info!("stdout: {}\nstderr: {}", stdout, stderr);
            if !success {
                return Ok(false);
            }
            // mkdir
            let (success, stdout, stderr) = sudo(&self.format_mkdir_parameters(username))?;
            info!("stdout: {}\nstderr: {}", stdout, stderr);
            if !success {
                return Ok(false);
            }
            // mount
            let (success, stdout, stderr) = sudo(&self.format_mount_parameters(username))?;
            info!("stdout: {}\nstderr: {}", stdout, stderr);
            if !success {
                return Ok(false);
            }
            // sudo -k
            sudo(&["-k".to_owned()])?;
            Ok(success)
        }
    }

    pub fn umount_close(
        &mut self,
        username: &str,
        passwords: &mut PasswordHolder,
    ) -> FmResult<bool> {
        // sudo
        let (success, _, _) = sudo_password(
            &["-S".to_owned(), "ls".to_owned(), "/root".to_owned()],
            &passwords.sudo()?,
        )?;
        if !success {
            return Ok(false);
        }
        // unmount
        let (success, stdout, stderr) = sudo(&self.format_umount_parameters(username))?;
        info!("stdout: {}\nstderr: {}", stdout, stderr);
        if !success {
            return Ok(false);
        }
        // close
        let (success, stdout, stderr) = sudo(&self.format_luksclose_parameters())?;
        info!("stdout: {}\nstderr: {}", stdout, stderr);
        if !success {
            return Ok(false);
        }
        // sudo -k
        let (success, stdout, stderr) = sudo(&["-k".to_owned()])?;
        info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    fn mount_point(&self) -> Option<String> {
        let system_info = System::new_all();
        system_info
            .disks()
            .iter()
            .map(|d| d.mount_point())
            .map(|p| p.to_str())
            .filter(|s| s.is_some())
            .map(|s| s.unwrap().to_owned())
            .filter(|s| s.contains(&self.uuid))
            .next()
    }

    pub fn as_string(&self) -> FmResult<String> {
        Ok(if let Some(mount_point) = self.mount_point() {
            format!("{} -> {}", self.path, mount_point)
        } else {
            format!("{} - not mounted", self.path)
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Device {
    pub cryptdevice: CryptoDevice,
    pub password_holder: PasswordHolder,
}

impl Device {
    pub fn from_line(line: &str) -> FmResult<Self> {
        Ok(Self {
            cryptdevice: CryptoDevice::from_line(line)?,
            password_holder: PasswordHolder::default(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeviceOpener {
    pub content: Vec<Device>,
    index: usize,
}

impl DeviceOpener {
    pub fn update(&mut self) -> FmResult<()> {
        self.content = filter_crypto_devices_lines(get_devices()?, "crypto")
            .iter()
            .map(|line| Device::from_line(line))
            .filter_map(|r| r.ok())
            .collect();
        self.index = 0;
        Ok(())
    }

    pub fn set_password(&mut self, password_kind: PasswordKind, password: String) {
        match password_kind {
            PasswordKind::SUDO => self.content[self.index].password_holder.set_sudo(password),
            PasswordKind::CRYPTSETUP => self.content[self.index]
                .password_holder
                .set_cryptsetup(password),
        }
    }

    pub fn mount_selected(&mut self) -> FmResult<()> {
        let username = current_username()?;
        let mut passwords = self.content[self.index].password_holder.clone();
        let success = self.content[self.index]
            .cryptdevice
            .open_mount(&username, &mut passwords)?;
        if !success {
            self.content[self.index].password_holder.reset();
            Self::reset_faillock()?
        }
        Self::drop_sudo()?;
        Ok(())
    }

    pub fn umount_selected(&mut self) -> FmResult<()> {
        let username = current_username()?;
        let mut passwords = self.content[self.index].password_holder.clone();
        let success = self.content[self.index]
            .cryptdevice
            .umount_close(&username, &mut passwords)?;
        if !success {
            self.content[self.index].password_holder.reset();
            Self::reset_faillock()?
        }
        Self::drop_sudo()?;
        Ok(())
    }

    fn reset_faillock() -> FmResult<()> {
        Command::new("faillock")
            .arg("--user")
            .arg(current_username()?)
            .arg("--reset")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }

    fn drop_sudo() -> FmResult<()> {
        Command::new("sudo")
            .arg("-k")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }

    pub fn has_sudo(&self) -> bool {
        self.content[self.index].password_holder.has_sudo()
    }

    pub fn has_cryptsetup(&self) -> bool {
        self.content[self.index].password_holder.has_cryptsetup()
    }
}

impl_selectable_content!(Device, DeviceOpener);
