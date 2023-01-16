use std::io::Write;
use std::process::{Command, Stdio};

use crate::fm_error::{FmError, FmResult};

#[derive(Default)]
pub struct PasswordHolder {
    sudo: Option<String>,
    cryptsetup: Option<String>,
}

impl PasswordHolder {
    pub fn set_sudo_password(&mut self, password: &str) {
        self.sudo = Some(password.to_owned())
    }

    pub fn set_cryptsetup_password(&mut self, passphrase: &str) {
        self.cryptsetup = Some(passphrase.to_owned())
    }

    pub fn cryptsetup(&self) -> FmResult<String> {
        Ok(self
            .cryptsetup
            .clone()
            .ok_or_else(|| FmError::custom("PasswordHolder", "sudo password isn't set"))?)
    }

    pub fn sudo(&self) -> FmResult<String> {
        Ok(self
            .sudo
            .clone()
            .ok_or_else(|| FmError::custom("PasswordHolder", "sudo password isn't set"))?)
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
pub fn filter_crypto_devices_lines(output: String) -> Vec<String> {
    output
        .lines()
        .filter(|line| line.contains("crypto"))
        .map(|line| line.into())
        .collect()
}

fn run_sudo_command_with_password(args: &[String], password: &str) -> FmResult<(String, String)> {
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

fn run_sudo_command(args: &[String]) -> FmResult<(String, String)> {
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

#[derive(Debug, Default)]
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

    pub fn is_already_mounted(&self) -> bool {
        self.mountpoints.is_some()
    }

    fn format_luksopen_parameters(&self) -> [String; 4] {
        [
            "cryptsetup".to_owned(),
            "luksOpen".to_owned(),
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

    fn format_mount_parameters(&self, username: &str) -> [String; 5] {
        [
            "mount".to_owned(),
            "-t".to_owned(),
            "ext4".to_owned(), // TODO! other fs ???
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
        if self.is_already_mounted() {
            Err(FmError::custom(
                "luks open mount",
                "device is already mounted",
            ))
        } else {
            // sudo
            run_sudo_command_with_password(
                &["-S".to_owned(), "ls".to_owned(), "/root".to_owned()],
                &passwords.sudo()?,
            )?;
            // open
            run_sudo_command_with_password(
                &self.format_luksopen_parameters(),
                &passwords.cryptsetup()?,
            )?;
            // mkdir
            run_sudo_command(&self.format_mkdir_parameters(username))?;
            // mount
            run_sudo_command(&self.format_mount_parameters(username))?;
            // sudo -t
            run_sudo_command(&["-t".to_owned()])?;
            self.update_from_line(&filter_crypto_devices_lines(get_devices()?)[0])?;
            Ok(self.is_already_mounted())
        }
    }

    pub fn umount_close(&mut self, username: &str, passwords: &PasswordHolder) -> FmResult<bool> {
        if self.is_already_mounted() {
            Err(FmError::custom("luks unmount close", "device isnt mounted"))
        } else {
            // sudo
            run_sudo_command_with_password(
                &["-S".to_owned(), "ls".to_owned(), "/root".to_owned()],
                &passwords.sudo()?,
            )?;
            // unmount
            run_sudo_command(&self.format_umount_parameters(username))?;
            // close
            run_sudo_command(&self.format_luksclose_parameters())?;
            // sudo -t
            run_sudo_command(&["-t".to_owned()])?;
            self.update_from_line(&filter_crypto_devices_lines(get_devices()?)[0])?;
            Ok(!self.is_already_mounted())
        }
    }
}
