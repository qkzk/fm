use anyhow::{anyhow, Result};
use log::info;

use crate::{
    mount_help::MountHelper,
    password::{sudo, sudo_password, PasswordHolder, PasswordKind},
};

/// Used to mount an iso file as a loop device.
/// Holds info about its source (`path`) and optional mountpoint (`mountpoints`).
/// Since it's used once and nothing can be done with it after mounting, it's dropped as soon as possible.
#[derive(Debug, Clone, Default)]
pub struct IsoDevice {
    /// The source, aka the iso file itself.
    pub path: String,
    /// None when creating, updated once the device is mounted.
    pub mountpoints: Option<std::path::PathBuf>,
    is_mounted: bool,
}

impl IsoDevice {
    const FILENAME: &str = "fm_iso";

    /// Creates a new instance from an iso file path.
    pub fn from_path(path: String) -> Self {
        log::info!("IsoDevice from_path: {path}");
        Self {
            path,
            ..Default::default()
        }
    }

    fn mountpoints(&self, username: &str) -> std::path::PathBuf {
        let mut mountpoint = std::path::PathBuf::from("/run/media");
        mountpoint.push(username);
        mountpoint.push(Self::FILENAME);
        mountpoint
    }
}

impl MountHelper for IsoDevice {
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            "mkdir".to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, Self::FILENAME),
        ]
    }

    fn format_mount_parameters(&mut self, username: &str) -> Vec<String> {
        let mountpoints = self.mountpoints(username);
        self.mountpoints = Some(mountpoints.clone());
        vec![
            "mount".to_owned(),
            "-o".to_owned(),
            "loop".to_owned(),
            self.path.clone(),
            mountpoints.to_string_lossy().to_string(),
        ]
    }

    fn format_umount_parameters(&self, username: &str) -> Vec<String> {
        vec![
            "umount".to_owned(),
            format!(
                "/run/media/{}/{}",
                username,
                self.mountpoints(username).display(),
            ),
        ]
    }

    fn is_mounted(&self) -> bool {
        self.is_mounted
    }

    fn umount(&mut self, username: &str, passwords: &mut PasswordHolder) -> Result<bool> {
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
        if success {
            self.is_mounted = false;
        }
        // sudo -k
        let (success, stdout, stderr) = sudo(&["-k".to_owned()])?;
        info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    fn mount(&mut self, username: &str, passwords: &mut PasswordHolder) -> Result<bool> {
        info!("iso mount: {username}, {passwords:?}");
        if self.is_mounted {
            Err(anyhow!("iso device mount: device is already mounted"))
        } else {
            // sudo
            let (success, _, _) = sudo_password(
                &["-S".to_owned(), "ls".to_owned(), "/root".to_owned()],
                &passwords.sudo()?,
            )?;
            if !success {
                return Ok(false);
            }
            // mkdir
            let (success, stdout, stderr) = sudo(&self.format_mkdir_parameters(username))?;
            info!("stdout: {}\nstderr: {}", stdout, stderr);
            let mut last_success = false;
            if success {
                // mount
                let (success, stdout, stderr) = sudo(&self.format_mount_parameters(username))?;
                last_success = success;
                info!("stdout: {}\nstderr: {}", stdout, stderr);
                // sudo -k
                self.is_mounted = success;
            } else {
                self.is_mounted = false;
            }
            sudo(&["-k".to_owned()])?;
            Ok(last_success)
        }
    }

    /// String representation of the device.
    fn as_string(&self) -> Result<String> {
        if let Some(ref mount_point) = self.mountpoints {
            Ok(format!(
                "mounted {}\nto {}",
                self.path,
                mount_point.display()
            ))
        } else {
            Ok(format!("not mounted {}", self.path))
        }
    }
}
