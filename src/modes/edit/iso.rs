use anyhow::{anyhow, Result};

use crate::io::{execute_sudo_command, set_sudo_session};
use crate::log_info;
use crate::modes::{MountCommands, MountParameters, MountRepr, PasswordHolder};

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
    const FILENAME: &'static str = "fm_iso";

    /// Creates a new instance from an iso file path.
    #[must_use]
    pub fn from_path(path: String) -> Self {
        log_info!("IsoDevice from_path: {path}");
        Self {
            path,
            ..Default::default()
        }
    }

    fn mountpoints(username: &str) -> std::path::PathBuf {
        let mut mountpoint = std::path::PathBuf::from("/run/media");
        mountpoint.push(username);
        mountpoint.push(Self::FILENAME);
        mountpoint
    }
}

impl MountParameters for IsoDevice {
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            "mkdir".to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, Self::FILENAME),
        ]
    }

    fn format_mount_parameters(&mut self, username: &str) -> Vec<String> {
        let mountpoints = Self::mountpoints(username);
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
                Self::mountpoints(username).display(),
            ),
        ]
    }
}

impl MountCommands for IsoDevice {
    fn is_mounted(&self) -> bool {
        self.is_mounted
    }

    fn umount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        // sudo
        let success = set_sudo_session(password)?;
        password.reset();
        if !success {
            return Ok(false);
        }
        // unmount
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_umount_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        if success {
            self.is_mounted = false;
        }
        // sudo -k
        let (success, stdout, stderr) = execute_sudo_command(&["-k"])?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    fn mount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        log_info!("iso mount: {username}, {password:?}");
        if self.is_mounted {
            Err(anyhow!("iso device mount: device is already mounted"))
        } else {
            // sudo
            let success = set_sudo_session(password)?;
            password.reset();
            if !success {
                return Ok(false);
            }
            // mkdir
            let (success, stdout, stderr) =
                execute_sudo_command(&self.format_mkdir_parameters(username))?;
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
            let mut last_success = false;
            if success {
                // mount
                let (success, stdout, stderr) =
                    execute_sudo_command(&self.format_mount_parameters(username))?;
                last_success = success;
                log_info!("stdout: {}\nstderr: {}", stdout, stderr);
                // sudo -k
                self.is_mounted = success;
            } else {
                self.is_mounted = false;
            }
            execute_sudo_command(&["-k"])?;
            Ok(last_success)
        }
    }
}

impl MountRepr for IsoDevice {
    /// String representation of the device.
    fn as_string(&self) -> Result<String> {
        self.mountpoints.as_ref().map_or_else(
            || Ok(format!("not mounted {}", self.path)),
            |mount_point| {
                Ok(format!(
                    "mounted {} to {}",
                    self.path,
                    mount_point.display()
                ))
            },
        )
    }

    fn device_name(&self) -> Result<String> {
        Ok(self.path.clone())
    }
}
