use anyhow::Result;

use ratatui::style::Style;

use crate::config::MENU_STYLES;
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

/// Methods used to display the mounted device in terminal
pub trait MountRepr: MountCommands {
    /// String representation of the device
    ///
    /// # Errors
    ///
    /// It may fail while parsing a path
    fn as_string(&self) -> Result<String>;

    /// Name of the device
    ///
    /// # Errors
    ///
    /// It may fail while accessing the device name
    fn device_name(&self) -> Result<String>;

    /// Default attr.
    /// Using configurable colors. "first" when mounted, "inert border" otherwise
    fn attr(&self) -> Style {
        if self.is_mounted() {
            MENU_STYLES.get().expect("Menu colors should be set").first
        } else {
            MENU_STYLES
                .get()
                .expect("Menu colors should be set")
                .inert_border
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
