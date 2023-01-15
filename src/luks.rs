use std::io::Write;
use std::process::{Command, Stdio};

use crate::fm_error::{FmError, FmResult};

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

#[derive(Debug, Default)]
pub struct CryptoDevice {
    fs_type: String,
    path: String,
    uuid: String,
    fs_ver: String,
    mountpoints: Option<String>,
    sudo_password: Option<String>,
    luks_passphrase: Option<String>,
}

impl CryptoDevice {
    /// Parse the output of a lsblk formated line into a struct
    pub fn from_line(line: &str) -> FmResult<Self> {
        let mut strings = line.split_whitespace();
        let mut params: Vec<Option<String>> = vec![None; 5];
        let mut count = 0;
        while let Some(param) = strings.next() {
            params[count] = Some(param.to_owned());
            count += 1;
        }
        Ok(Self {
            fs_type: params
                .remove(0)
                .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?,
            path: params
                .remove(0)
                .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?,
            uuid: params
                .remove(0)
                .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?,
            fs_ver: params
                .remove(0)
                .ok_or_else(|| FmError::custom("CryptoDevice", "parameter shouldn't be None"))?,
            mountpoints: params.remove(0),
            sudo_password: None,
            luks_passphrase: None,
        })
    }

    pub fn is_already_mounted(&self) -> bool {
        self.mountpoints.is_some()
    }

    pub fn format_luksopen_parameters(&self) -> [String; 5] {
        [
            "-S".to_owned(),
            "cryptsetup".to_owned(),
            "luksOpen".to_owned(),
            self.path.clone(),
            self.uuid.clone(),
        ]
    }

    pub fn format_mkdir_parameters(&self, username: &str) -> [String; 4] {
        [
            "-S".to_owned(),
            "mkdir".to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.uuid),
        ]
    }

    pub fn format_mount_parameters(&self, username: &str) -> [String; 6] {
        [
            "-S".to_owned(),
            "cryptsetup".to_owned(),
            "-t".to_owned(),
            "ext4".to_owned(), // TODO! other fs ???
            format!("/dev/mapper/{}", self.uuid),
            format!("/run/media/mapper/{}/{}", username, self.uuid),
        ]
    }

    pub fn format_umount_parameters(&self, username: &str) -> [String; 1] {
        [format!("/run/media/mapper/{}/{}", username, self.uuid)]
    }

    pub fn format_luksclose_parameters(&self) -> [String; 1] {
        [self.uuid.to_owned()]
    }

    pub fn set_sudo_password(&mut self, password: &str) {
        self.sudo_password = Some(password.to_owned())
    }

    pub fn set_luks_passphrase(&mut self, passphrase: &str) {
        self.luks_passphrase = Some(passphrase.to_owned())
    }

    pub fn open_mount(&self, username: &str) -> FmResult<()> {
        if self.is_already_mounted() {
            Err(FmError::custom(
                "luks open mount",
                "device is already mounted",
            ))
        } else if let Some(password) = &self.sudo_password {
            if let Some(passphrase) = &self.luks_passphrase {
                let password = password.to_owned();
                let password2 = password.clone();
                let passphrase = passphrase.to_owned();
                let passphrase2 = passphrase.clone();

                let mut child = Command::new("sudo")
                    .args(&self.format_luksopen_parameters())
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()?;
                let mut stdin = child.stdin.take().expect("Failed to open stdin");
                std::thread::spawn(move || {
                    stdin
                        .write_all(format!("{}\n{}", &password, &passphrase).as_bytes())
                        .expect("Failed to write to stdin");
                });
                child.wait_with_output()?;

                let mut child = Command::new("sudo")
                    .args(&self.format_mkdir_parameters(username))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()?;
                let mut stdin = child.stdin.take().expect("Failed to open stdin");
                std::thread::spawn(move || {
                    stdin
                        .write_all(password2.as_bytes())
                        .expect("Failed to write to stdin");
                });
                child.wait_with_output()?;

                let mut child = Command::new("sudo")
                    .args(&self.format_mount_parameters(username))
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()?;
                let mut stdin = child.stdin.take().expect("Failed to open stdin");
                std::thread::spawn(move || {
                    stdin
                        .write_all(passphrase2.as_bytes())
                        .expect("Failed to write to stdin");
                });
                child.wait_with_output()?;

                Ok(())
            } else {
                Err(FmError::custom(
                    "luks open mount",
                    "missing a password or passphrase",
                ))
            }
        } else {
            Err(FmError::custom(
                "luks open mount",
                "missing a password or passphrase",
            ))
        }
    }
}
