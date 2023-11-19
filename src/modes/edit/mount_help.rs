use anyhow::Result;

use crate::modes::PasswordHolder;

/// Bunch of methods used to mount / unmount a block device or a device image file.
pub trait MountCommands {
    /// True if the device is mounted
    fn is_mounted(&self) -> bool;

    /// Mount the device
    fn mount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool>;

    /// Unmount the device
    fn umount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool>;
}

/// Methods used to display the mounted device in terminal
pub trait MountRepr: MountCommands {
    /// String representation of the device
    fn as_string(&self) -> Result<String>;

    /// Name of the device
    fn device_name(&self) -> Result<String>;

    /// Default attr.
    /// Foreground is blue when device is mounted, white otherwise.
    fn attr(&self) -> tuikit::attr::Attr {
        if self.is_mounted() {
            tuikit::attr::Attr::from(tuikit::attr::Color::BLUE)
        } else {
            tuikit::attr::Attr::default()
        }
    }
}

/// Parameters used to create the commands : mkdir, mount & umount.
pub trait MountParameters {
    /// Parameters used to `sudo mkdir mountpoint`
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3];

    /// Parameters used to mount the device
    fn format_mount_parameters(&mut self, username: &str) -> Vec<String>;

    /// Parameters used to umount the device
    fn format_umount_parameters(&self, username: &str) -> Vec<String>;
}
