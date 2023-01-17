use std::io::Write;
use std::process::{Command, Stdio};

use crate::fm_error::{FmError, FmResult};
use crate::impl_selectable_content;
use crate::utils::current_username;

#[derive(Debug, Clone, Copy)]
pub enum PasswordKind {
    SUDO,
    CRYPTSETUP,
}

impl std::fmt::Display for PasswordKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let asker = match self {
            Self::SUDO => "sudo",
            Self::CRYPTSETUP => "cryptsetup",
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

    pub fn can_mount(&self) -> bool {
        self.sudo.is_some() && self.cryptsetup.is_some()
    }

    pub fn can_umount(&self) -> bool {
        self.sudo.is_some()
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
fn sudo_password(args: &[String], password: &str) -> FmResult<(String, String)> {
    println!("sudo {:?}", args);
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
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}

/// Run a passwordless sudo command.
/// Returns stdout & stderr
fn sudo(args: &[String]) -> FmResult<(String, String)> {
    println!("sudo {:?}", args);
    let child = Command::new("sudo")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let output = child.wait_with_output()?;
    Ok((
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

    pub fn open_mount(&mut self, username: &str, passwords: &PasswordHolder) -> FmResult<bool> {
        if self.is_mounted()? {
            Err(FmError::custom(
                "luks open mount",
                "device is already mounted",
            ))
        } else {
            // sudo
            let output = sudo_password(
                &["-S".to_owned(), "ls".to_owned(), "/root".to_owned()],
                &passwords.sudo()?,
            )?;
            println!("stdout: {}\nstderr: {}", output.0, output.1);
            // open
            let output =
                sudo_password(&self.format_luksopen_parameters(), &passwords.cryptsetup()?)?;
            println!("stdout: {}\nstderr: {}", output.0, output.1);
            // mkdir
            let output = sudo(&self.format_mkdir_parameters(username))?;
            println!("stdout: {}\nstderr: {}", output.0, output.1);
            // mount
            let output = sudo(&self.format_mount_parameters(username))?;
            println!("stdout: {}\nstderr: {}", output.0, output.1);
            // sudo -t
            sudo(&["-k".to_owned()])?;
            println!("wait a few seconds...");
            std::thread::sleep(std::time::Duration::from_secs(10));
            self.is_mounted()
        }
    }

    pub fn umount_close(&mut self, username: &str, passwords: &PasswordHolder) -> FmResult<bool> {
        // sudo
        let output = sudo_password(
            &["-S".to_owned(), "ls".to_owned(), "/root".to_owned()],
            &passwords.sudo()?,
        )?;
        println!("stdout: {}\nstderr: {}", output.0, output.1);
        // unmount
        let output = sudo(&self.format_umount_parameters(username))?;
        println!("stdout: {}\nstderr: {}", output.0, output.1);
        // close
        let output = sudo(&self.format_luksclose_parameters())?;
        println!("stdout: {}\nstderr: {}", output.0, output.1);
        // sudo -t
        let output = sudo(&["-k".to_owned()])?;
        println!("stdout: {}\nstderr: {}", output.0, output.1);
        println!("wait a few seconds...");
        std::thread::sleep(std::time::Duration::from_secs(5));
        Ok(!self.is_mounted()?)
    }

    pub fn is_mounted(&self) -> FmResult<bool> {
        let mut block = Self::default();
        block.update_from_line(&filter_crypto_devices_lines(get_devices()?, &self.uuid)[0])?;
        Ok(block.mountpoints.is_some())
    }

    pub fn is_opened(&self) -> FmResult<bool> {
        Ok(true)
    }

    pub fn as_string(&self) -> FmResult<String> {
        let is_mounted = self.is_mounted()?;
        let mounted_char = if is_mounted { 'm' } else { 'u' };
        let opened_char = if self.is_opened()? { 'o' } else { 'c' };
        let address = "/dev/sdb";
        let mut s = format!("{} {} {}", mounted_char, opened_char, address);

        if let Some(mountpoints) = &self.mountpoints {
            s.push_str(" -> ");
            s.push_str(&mountpoints);
        }

        Ok(s)
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
        let passwords = self.content[self.index].password_holder.clone();
        self.content[self.index]
            .cryptdevice
            .open_mount(&username, &passwords)?;
        Ok(())
    }

    pub fn umount_selected(&mut self) -> FmResult<()> {
        let username = current_username()?;
        let passwords = self.content[self.index].password_holder.clone();
        self.content[self.index]
            .cryptdevice
            .umount_close(&username, &passwords)?;
        Ok(())
    }

    pub fn can_mount(&self) -> bool {
        self.content[self.index].password_holder.can_mount()
    }

    pub fn can_umount(&self) -> bool {
        self.content[self.index].password_holder.can_umount()
    }
}

impl_selectable_content!(Device, DeviceOpener);
