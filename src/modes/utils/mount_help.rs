use anyhow::Result;

use crate::modes::PasswordHolder;

/// Bunch of methods used to mount / unmount a block device or a device image file.
pub trait MountCommands {
    /// True if the device is mounted
    fn is_mounted(&self) -> bool;

    /// Mount the device
    ///
    /// # Errors
    ///
    /// It may fail if mounting returns an error
    fn mount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool>;

    /// Unmount the device
    ///
    /// # Errors
    ///
    /// It may fail if unmounting returns an error
    fn umount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool>;
}

/// Parameters used to create the commands : mkdir, mount & umount.
pub trait MountParameters {
    /// Parameters used to `sudo mkdir mountpoint`
    fn mkdir_parameters(&self, username: &str) -> [String; 3];

    /// Parameters used to mount the device
    fn mount_parameters(&self, username: &str) -> Vec<String>;

    /// Parameters used to umount the device
    fn umount_parameters(&self, username: &str) -> Vec<String>;
}
