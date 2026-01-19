use std::{
    borrow::Cow,
    cmp::min,
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
};

use anyhow::{bail, Context, Result};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Style},
    text::Text,
    widgets::{Cell, Row, Table},
    Frame,
};
use serde::Deserialize;
use serde_json::{from_str, from_value, Value};
use sysinfo::Disks;

use crate::common::{
    current_uid, current_username, is_dir_empty, is_in_path, CRYPTSETUP, GIO, LSBLK, MKDIR, MOUNT,
    UDISKSCTL, UMOUNT,
};
use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::{
    drop_sudo_privileges, execute_and_output, execute_sudo_command_passwordless,
    execute_sudo_command_with_password, reset_sudo_faillock, set_sudo_session, CowStr, DrawMenu,
    Offseted,
};
use crate::modes::{ContentWindow, MountCommands, MountParameters, PasswordHolder};
use crate::{colored_skip_take, impl_content, impl_selectable, log_info, log_line};

/// Used to mount an iso file as a loop device.
/// Holds info about its source (`path`) and optional mountpoint (`mountpoints`).
/// Since it's used once and nothing can be done with it after mounting, it's dropped as soon as possible.
#[derive(Debug, Clone, Default)]
pub struct IsoDevice {
    /// The source, aka the iso file itself.
    pub path: String,
    /// None when creating, updated once the device is mounted.
    pub mountpoints: Option<String>,
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

    fn mountpoints(username: &str) -> String {
        format!(
            "/run/media/{username}/{filename}",
            filename = Self::FILENAME
        )
    }

    fn set_mountpoint(&mut self, username: &str) {
        self.mountpoints = Some(Self::mountpoints(username))
    }
}

impl MountParameters for IsoDevice {
    fn mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            "mkdir".to_owned(),
            "-p".to_owned(),
            format!(
                "/run/media/{username}/{filename}",
                filename = Self::FILENAME
            ),
        ]
    }

    fn mount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            "mount".to_owned(),
            "-o".to_owned(),
            "loop".to_owned(),
            self.path.clone(),
            self.mountpoints
                .clone()
                .expect("mountpoint should be set already"),
        ]
    }

    fn umount_parameters(&self, username: &str) -> Vec<String> {
        vec![
            "umount".to_owned(),
            format!(
                "/run/media/{username}/{mountpoint}",
                mountpoint = Self::mountpoints(username),
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
        let (success, stdout, stderr) =
            execute_sudo_command_passwordless(&self.umount_parameters(username))?;
        log_info!("stdout: {stdout}\nstderr: {stderr}");
        if success {
            self.is_mounted = false;
        }
        drop_sudo_privileges()?;
        Ok(success)
    }

    fn mount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        log_info!("iso mount: {username}, {password:?}");
        if self.is_mounted {
            bail!("iso device mount: device is already mounted")
        };
        // sudo
        let success = set_sudo_session(password)?;
        password.reset();
        if !success {
            return Ok(false);
        }
        // mkdir
        let (success, stdout, stderr) =
            execute_sudo_command_passwordless(&self.mkdir_parameters(username))?;
        if !stdout.is_empty() || !stderr.is_empty() {
            log_info!("stdout: {stdout}\nstderr: {stderr}");
        }
        let mut last_success = false;
        if success {
            self.set_mountpoint(username);
            // mount
            let (success, stdout, stderr) =
                execute_sudo_command_passwordless(&self.mount_parameters(username))?;
            last_success = success;
            if !success {
                log_info!("stdout: {stdout}\nstderr: {stderr}");
            }
            self.is_mounted = success;
        } else {
            self.is_mounted = false;
        }
        drop_sudo_privileges()?;
        Ok(last_success)
    }
}

impl Display for IsoDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.mountpoints {
            Some(mountpoint) => write!(f, "mounted {path} to {mountpoint}", path = self.path,),
            None => write!(f, "not mounted {path}", path = self.path),
        }
    }
}

/// Possible actions on mountable devices
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MountAction {
    MOUNT,
    UMOUNT,
}

#[derive(Debug)]
pub enum NetworkKind {
    NFS,
    CIFS,
}

impl NetworkKind {
    fn from_fs_type(fs_type: &str) -> Option<Self> {
        match fs_type {
            "cifs" => Some(Self::CIFS),
            "nfs4" => Some(Self::NFS),
            _ => None,
        }
    }
}

impl Display for NetworkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            Self::NFS => "nfs",
            Self::CIFS => "smb",
        };

        write!(f, "{kind}")
    }
}

/// A mounted device from a remote location.
/// Only NTFS & CIFS are supported ATM.
#[derive(Debug)]
pub struct NetworkMount {
    pub kind: NetworkKind,
    pub path: String,
    pub mountpoint: String,
}

/// Holds a network mount point.
impl NetworkMount {
    /// Returns a `NetWorkMount` parsed from a line of /proc/self/mountinfo
    /// 96 29 0:60 / /home/user/nfs rw,relatime shared:523 - nfs4 hostname:/remote/path rw,vers=4.2,rsize=524288,wsize=524288,namlen=255,hard,proto=tcp,timeo=900,retrans=5,sec=sys,clientaddr=192.168.1.17,local_lock=none,addr=remote_ip
    /// 483 29 0:73 / /home/user/cifs rw,relatime shared:424 - cifs //ip_adder/qnas rw,vers=3.1.1,cache=strict,username=quentin,uid=0,noforceuid,gid=0,noforcegid,addr=yout_ip,file_mode=0755,dir_mode=0755,soft,nounix,serverino,mapposix,reparse=nfs,rsize=4194304,wsize=4194304,bsize=1048576,retrans=1,echo_interval=60,actimeo=1,closetimeo=1
    fn from_network_line(line: io::Result<String>) -> Option<Self> {
        let Ok(line) = line else {
            return None;
        };
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() <= 6 {
            return None;
        }
        let kind = NetworkKind::from_fs_type(parts.get(parts.len() - 3)?)?;
        let mountpoint = parts.get(4)?.to_string();
        let path = parts.get(parts.len() - 2)?.to_string();
        Some(Self {
            kind,
            mountpoint,
            path,
        })
    }

    fn umount(&self, password: &mut PasswordHolder) -> Result<bool> {
        let success = set_sudo_session(password);
        password.reset();
        if !matches!(success, Ok(true)) {
            return Ok(false);
        }
        let (success, _, _) =
            execute_sudo_command_passwordless(&[UMOUNT, self.mountpoint.as_str()])?;
        log_info!(
            "Unmounted {device}. Success ? {success}",
            device = self.mountpoint,
        );
        drop_sudo_privileges()?;
        Ok(success)
    }

    fn symbols(&self) -> String {
        " MN".to_string()
    }
}

impl Display for NetworkMount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MN {kind} {path} -> {mountpoint}",
            kind = self.kind,
            path = self.path,
            mountpoint = self.mountpoint
        )
    }
}

/// Holds a MTP device name, a path and a flag set to true
/// if the device is mounted.
#[derive(Debug, Clone, Default)]
pub struct Mtp {
    pub name: String,
    pub path: String,
    pub is_mounted: bool,
    pub is_ejected: bool,
}

impl Mtp {
    /// Creates a `Removable` instance from a filtered `gio` command output.
    ///
    /// `gio mount -l`  will return a lot of information about mount points,
    /// including MTP (aka Android) devices.
    /// We don't check if the device actually exists, we just create the instance.
    fn from_gio(line: &str) -> Result<Self> {
        let name = line
            .replace("activation_root=mtp://", "")
            .replace('/', "")
            .trim()
            .to_owned();
        let uid = current_uid()?;
        let path = format!("/run/user/{uid}/gvfs/mtp:host={name}");
        let pb_path = std::path::Path::new(&path);
        let is_mounted = pb_path.exists() && !is_dir_empty(pb_path)?;
        let is_ejected = false;
        #[cfg(debug_assertions)]
        log_info!("gio {name} - is_mounted {is_mounted}");
        Ok(Self {
            name,
            path,
            is_mounted,
            is_ejected,
        })
    }

    /// Format itself as a valid `gio mount $device` argument.
    fn format_for_gio(&self) -> String {
        format!("mtp://{name}", name = self.name)
    }

    /// True if the device is mounted
    fn is_mounted(&self) -> bool {
        self.is_mounted
    }

    /// Mount a non mounted removable device.
    /// `Err` if the device is already mounted.
    /// Runs a `gio mount $name` command and check
    /// the result.
    /// The `is_mounted` flag is updated accordingly to the result.
    fn mount(&mut self) -> Result<bool> {
        if self.is_mounted {
            bail!("Already mounted {name}", name = self.name);
        }
        self.is_mounted = execute_and_output(GIO, ["mount", &self.format_for_gio()])?
            .status
            .success();

        log_line!(
            "Mounted {device}. Success ? {success}",
            device = self.name,
            success = self.is_mounted
        );
        Ok(self.is_mounted)
    }

    /// Unount a mounted removable device.
    /// `Err` if the device isnt mounted.
    /// Runs a `gio mount $device_name` command and check
    /// the result.
    /// The `is_mounted` flag is updated accordingly to the result.
    fn umount(&mut self) -> Result<bool> {
        if !self.is_mounted {
            bail!("Not mounted {name}", name = self.name);
        }
        self.is_mounted = execute_and_output(GIO, ["mount", &self.format_for_gio(), "-u"])?
            .status
            .success();

        log_info!(
            "Unmounted {device}. Success ? {success}",
            device = self.name,
            success = self.is_mounted
        );
        Ok(!self.is_mounted)
    }

    fn symbols(&self) -> String {
        let is_mounted = self.is_mounted();
        let mount_repr = if is_mounted { 'M' } else { 'U' };
        format!(" {mount_repr}P")
    }
}

impl Display for Mtp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let is_mounted = self.is_mounted();
        write!(
            f,
            "{mount_repr}P {name}",
            mount_repr = if is_mounted { 'M' } else { 'U' },
            name = self.name.clone()
        )?;
        if is_mounted {
            write!(f, " -> {path}", path = self.path)?;
        }
        Ok(())
    }
}

/// Encrypted devices which can be mounted.
/// Mounting an encrypted device requires a password.
#[derive(Debug)]
pub struct EncryptedBlockDevice {
    pub path: String,
    pub uuid: Option<String>,
    mountpoint: Option<String>,
    label: Option<String>,
    model: Option<String>,
    parent: Option<String>,
}

impl MountParameters for EncryptedBlockDevice {
    fn mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            MKDIR.to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.uuid.clone().unwrap()),
        ]
    }

    fn mount_parameters(&self, username: &str) -> Vec<String> {
        vec![
            MOUNT.to_owned(),
            format!("/dev/mapper/{}", self.uuid.clone().unwrap()),
            format!("/run/media/{}/{}", username, self.uuid.clone().unwrap()),
        ]
    }

    fn umount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            UDISKSCTL.to_owned(),
            "unmount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }
}

impl Display for EncryptedBlockDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{is_mounted}C {path} {label}",
            is_mounted = if self.is_mounted() { 'M' } else { 'U' },
            label = self.label_repr(),
            path = truncate_string(&self.path, 20)
        )?;
        if let Some(mountpoint) = &self.mountpoint {
            write!(f, " -> {mp}", mp = truncate_string(mountpoint, 25))?;
        }
        Ok(())
    }
}

impl From<BlockDevice> for EncryptedBlockDevice {
    fn from(device: BlockDevice) -> Self {
        EncryptedBlockDevice {
            path: device.path,
            uuid: device.uuid,
            mountpoint: device.mountpoint,
            label: device.label,
            model: device.model,
            parent: None,
        }
    }
}
impl EncryptedBlockDevice {
    fn set_parent(&mut self, parent_uuid: &Option<String>) {
        self.parent = parent_uuid.clone()
    }

    pub fn mount(&self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        let success = is_in_path(CRYPTSETUP)
            && self.set_sudo_session(password)?
            && self.execute_luks_open(password)?
            && self.execute_mkdir_crypto(username)?
            && self.execute_mount_crypto(username)?;
        drop_sudo_privileges()?;
        Ok(success)
    }

    fn set_sudo_session(&self, password: &mut PasswordHolder) -> Result<bool> {
        if !set_sudo_session(password)? {
            password.reset();
            return Ok(false);
        }
        Ok(true)
    }

    fn execute_luks_open(&self, password: &mut PasswordHolder) -> Result<bool> {
        match execute_sudo_command_with_password(
            &self.format_luksopen_parameters(),
            password
                .cryptsetup()
                .as_ref()
                .context("cryptsetup password_holder isn't set")?,
            std::path::Path::new("/"),
        ) {
            Ok((success, stdout, stderr)) => {
                log_info!("stdout: {stdout}\nstderr: {stderr}");
                password.reset();
                Ok(success)
            }
            Err(error) => {
                password.reset();
                Err(error)
            }
        }
    }

    fn execute_mkdir_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command_passwordless(&self.mkdir_parameters(username))?;
        log_info!("stdout: {stdout}\nstderr: {stderr}");
        Ok(success)
    }

    fn execute_mount_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command_passwordless(&self.mount_parameters(username))?;
        log_info!("stdout: {stdout}\nstderr: {stderr}");
        Ok(success)
    }

    pub fn umount_close_crypto(
        &self,
        username: &str,
        password_holder: &mut PasswordHolder,
    ) -> Result<bool> {
        let success = is_in_path(CRYPTSETUP)
            && self.set_sudo_session(password_holder)?
            && self.execute_umount_crypto(username)?
            && self.execute_luks_close()?;
        drop_sudo_privileges()?;
        password_holder.reset();
        Ok(success)
    }

    fn execute_umount_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command_passwordless(&self.umount_parameters(username))?;
        if !success {
            log_info!("stdout: {stdout}\nstderr: {stderr}");
        }
        Ok(success)
    }

    fn execute_luks_close(&self) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command_passwordless(&self.format_luksclose_parameters())?;
        if !success {
            log_info!("stdout: {stdout}\nstderr: {stderr}");
        }
        Ok(success)
    }

    fn format_luksopen_parameters(&self) -> [String; 4] {
        [
            CRYPTSETUP.to_owned(),
            "open".to_owned(),
            self.path.clone(),
            self.uuid.clone().unwrap(),
        ]
    }

    fn format_luksclose_parameters(&self) -> [String; 3] {
        [
            CRYPTSETUP.to_owned(),
            "close".to_owned(),
            self.parent.clone().unwrap(),
        ]
    }

    const fn is_crypto(&self) -> bool {
        true
    }

    fn label_repr(&self) -> &str {
        if let Some(label) = &self.label {
            label
        } else if let Some(model) = &self.model {
            model
        } else {
            ""
        }
    }

    /// True if there's a mount point for this drive.
    /// It's only valid if we mounted the device since it requires
    /// the uuid to be in the mount point.
    fn is_mounted(&self) -> bool {
        self.mountpoint.is_some()
    }

    fn symbols(&self) -> String {
        format!(
            " {is_mounted}C",
            is_mounted = if self.is_mounted() { "M" } else { "U" }
        )
    }
}

/// A device which can be mounted.
/// Default "mountable" struct for any kind of device **except** encrypted devices.
/// They require special methods to be mounted since it requires a password which
/// can't be provided from here.
#[derive(Default, Deserialize, Debug)]
pub struct BlockDevice {
    fstype: Option<String>,
    pub path: String,
    uuid: Option<String>,
    mountpoint: Option<String>,
    name: Option<String>,
    label: Option<String>,
    hotplug: bool,
    model: Option<String>,
    #[serde(default)]
    children: Vec<BlockDevice>,
}

impl BlockDevice {
    fn device_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| self.uuid.as_ref().unwrap().clone())
    }

    fn mount_no_password(&self) -> Result<bool> {
        let mut args = self.mount_parameters("");
        let output = execute_and_output(&args.remove(0), &args)?;
        Ok(output.status.success())
    }

    fn umount_no_password(&self) -> Result<bool> {
        let mut args = self.umount_parameters("");
        let output = execute_and_output(&args.remove(0), &args)?;
        Ok(output.status.success())
    }

    fn is_crypto(&self) -> bool {
        let Some(fstype) = &self.fstype else {
            return false;
        };
        fstype.contains("crypto")
    }

    fn is_loop(&self) -> bool {
        self.path.contains("loop")
    }

    fn prefix_repr(&self) -> &str {
        match (self.is_loop(), self.hotplug) {
            (true, _) => "L",
            (false, true) => "R",
            _ => " ",
        }
    }

    fn label_repr(&self) -> &str {
        if let Some(label) = &self.label {
            label
        } else if let Some(model) = &self.model {
            model
        } else {
            ""
        }
    }

    fn symbols(&self) -> String {
        format!(
            " {is_mounted}{prefix}",
            is_mounted = if self.is_mounted() { "M" } else { "U" },
            prefix = self.prefix_repr()
        )
    }

    pub fn try_power_off(&self) -> Result<bool> {
        if !self.hotplug && !self.is_mounted() {
            return Ok(false);
        }
        let output = execute_and_output(UDISKSCTL, ["power-off", "-b", &self.path])?;
        Ok(output.status.success())
    }
}

impl MountParameters for BlockDevice {
    fn mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            MKDIR.to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.device_name()),
        ]
    }

    fn mount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            UDISKSCTL.to_owned(),
            "mount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }

    fn umount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            UDISKSCTL.to_owned(),
            "unmount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }
}

impl MountCommands for BlockDevice {
    /// True if there's a mount point for this drive.
    /// It's only valid if we mounted the device since it requires
    /// the uuid to be in the mount point.
    fn is_mounted(&self) -> bool {
        self.mountpoint.is_some()
    }

    fn mount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        // sudo
        let success = set_sudo_session(password)?;
        password.reset();
        if !success {
            return Ok(false);
        }
        // mount
        let args_sudo = self.mount_parameters(username);
        let (success, stdout, stderr) = execute_sudo_command_passwordless(&args_sudo)?;
        if !success {
            log_info!("stdout: {stdout}\nstderr: {stderr}");
            return Ok(false);
        }
        if !success {
            reset_sudo_faillock()?;
        }
        Ok(success)
    }

    fn umount(&mut self, username: &str, password: &mut PasswordHolder) -> Result<bool> {
        let success = set_sudo_session(password)?;
        password.reset();
        if !success {
            return Ok(false);
        }
        let (success, stdout, stderr) =
            execute_sudo_command_passwordless(&self.umount_parameters(username))?;
        if !success {
            log_info!("stdout: {stdout}\nstderr: {stderr}");
        }
        Ok(success)
    }
}

impl Display for BlockDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{is_mounted}{prefix} {path} {label}",
            is_mounted = if self.is_mounted() { "M" } else { "U" },
            prefix = self.prefix_repr(),
            label = self.label_repr(),
            path = self.path
        )?;
        if let Some(mountpoint) = &self.mountpoint {
            write!(f, " -> {mountpoint}")?;
        }
        Ok(())
    }
}

/// A mounted partition using sshfs.
#[derive(Debug)]
pub struct RemoteDevice {
    name: String,
    mountpoint: String,
}

impl RemoteDevice {
    fn new<S, T>(name: S, mountpoint: T) -> Self
    where
        S: Into<String>,
        T: Into<String>,
    {
        Self {
            name: name.into(),
            mountpoint: mountpoint.into(),
        }
    }

    const fn is_mounted(&self) -> bool {
        true
    }

    fn symbols(&self) -> String {
        " MR".to_string()
    }
}

/// A mountable device which can be of many forms.
#[derive(Debug)]
pub enum Mountable {
    Device(BlockDevice),
    Encrypted(EncryptedBlockDevice),
    MTP(Mtp),
    Remote(RemoteDevice),
    Network(NetworkMount),
}
impl Display for Mountable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Device(device) => write!(f, "{device}"),
            Self::Encrypted(device) => write!(f, "{device}"),
            Self::MTP(device) => write!(f, "{device}"),
            Self::Network(device) => write!(f, "{device}"),
            Self::Remote(RemoteDevice { name, mountpoint }) => {
                write!(f, "MS {name} -> {mountpoint}",)
            }
        }
    }
}

impl Mountable {
    pub fn is_crypto(&self) -> bool {
        match &self {
            Self::Device(device) => device.is_crypto(),
            Self::Encrypted(device) => device.is_crypto(),
            Self::Network(_) => false,
            Self::MTP(_) => false,
            Self::Remote(_) => false,
        }
    }

    pub fn is_mounted(&self) -> bool {
        match &self {
            Self::Device(device) => device.is_mounted(),
            Self::Encrypted(device) => device.is_mounted(),
            Self::MTP(device) => device.is_mounted(),
            Self::Network(_) => true,
            Self::Remote(device) => device.is_mounted(),
        }
    }

    fn path(&self) -> &str {
        match &self {
            Self::Device(device) => device.path.as_str(),
            Self::Encrypted(device) => device.path.as_str(),
            Self::MTP(device) => device.path.as_str(),
            Self::Network(device) => device.path.as_str(),
            Self::Remote(RemoteDevice {
                name: _,
                mountpoint,
            }) => mountpoint.as_str(),
        }
    }

    pub fn path_repr(&self) -> String {
        truncate_string(self.path(), 25)
    }

    fn mountpoint(&self) -> Option<&str> {
        match self {
            Mountable::Device(device) => device.mountpoint.as_deref(),
            Mountable::Encrypted(device) => device.mountpoint.as_deref(),
            Mountable::MTP(device) => Some(&device.path),
            Mountable::Network(device) => Some(&device.mountpoint),
            Mountable::Remote(RemoteDevice {
                name: _,
                mountpoint,
            }) => Some(mountpoint),
        }
    }

    pub fn mountpoint_repr(&self) -> &str {
        self.mountpoint().unwrap_or_default()
    }

    pub fn symbols(&self) -> String {
        match &self {
            Self::Device(device) => device.symbols(),
            Self::Encrypted(device) => device.symbols(),
            Self::MTP(device) => device.symbols(),
            Self::Network(device) => device.symbols(),
            Self::Remote(device) => device.symbols(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Device(device) => device.label_repr().to_string(),
            Self::Encrypted(device) => device.label_repr().to_string(),
            Self::MTP(_) => "".to_string(),
            Self::Network(device) => device.kind.to_string(),
            Self::Remote(_) => "".to_string(),
        }
    }
}

impl CowStr for Mountable {
    fn cow_str(&self) -> Cow<'_, str> {
        self.to_string().into()
    }
}
struct MountBuilder;

impl MountBuilder {
    fn build_from_json() -> Result<Vec<Mountable>> {
        let json_content = get_devices_json()?;
        match Self::from_json(json_content) {
            Ok(content) => Ok(content),
            Err(e) => {
                log_info!("update error {e:#?}");
                Ok(vec![])
            }
        }
    }

    fn from_json(json_content: String) -> Result<Vec<Mountable>, Box<dyn std::error::Error>> {
        let devices: Vec<BlockDevice> = Self::read_blocks_from_json(json_content)?;
        let mut content = vec![];
        for parent in devices.into_iter() {
            let is_crypto = parent.is_crypto();
            if !parent.children.is_empty() {
                Self::push_children(is_crypto, &mut content, parent);
            } else if parent.uuid.is_some() {
                Self::push_parent(is_crypto, &mut content, parent)
            }
        }
        Ok(content)
    }

    fn read_blocks_from_json(
        json_content: String,
    ) -> Result<Vec<BlockDevice>, Box<dyn std::error::Error>> {
        let mut value: Value = from_str(&json_content)?;

        let blockdevices_value: Value = value
            .get_mut("blockdevices")
            .ok_or("Missing 'blockdevices' field in JSON")?
            .take();
        Ok(from_value(blockdevices_value)?)
    }

    fn push_children(is_crypto: bool, content: &mut Vec<Mountable>, parent: BlockDevice) {
        for mut children in parent.children.into_iter() {
            if is_crypto {
                let mut encrypted_children: EncryptedBlockDevice = children.into();
                encrypted_children.set_parent(&parent.uuid);
                content.push(Mountable::Encrypted(encrypted_children));
            } else {
                children.model = parent.model.clone();
                content.push(Mountable::Device(children));
            }
        }
    }

    fn push_parent(is_crypto: bool, content: &mut Vec<Mountable>, parent: BlockDevice) {
        if is_crypto {
            content.push(Mountable::Encrypted(parent.into()))
        } else {
            content.push(Mountable::Device(parent))
        }
    }

    fn extend_with_remote(content: &mut Vec<Mountable>, disks: &Disks) {
        content.extend(
            disks
                .iter()
                .filter(|d| d.file_system().to_string_lossy().contains("sshfs"))
                .map(|d| {
                    Mountable::Remote(RemoteDevice::new(
                        d.name().to_string_lossy(),
                        d.mount_point().to_string_lossy(),
                    ))
                })
                .collect::<Vec<_>>(),
        );
    }

    fn extend_with_network(content: &mut Vec<Mountable>) -> Result<()> {
        content.extend(Self::get_network_devices()?);
        Ok(())
    }

    fn get_network_devices() -> io::Result<Vec<Mountable>> {
        let reader = BufReader::new(File::open("/proc/self/mountinfo")?);
        let mut network_mountables = vec![];

        for line in reader.lines() {
            let Some(network_mount) = NetworkMount::from_network_line(line) else {
                continue;
            };
            network_mountables.push(Mountable::Network(network_mount));
        }
        Ok(network_mountables)
    }

    fn extend_with_mtp_from_gio(content: &mut Vec<Mountable>) {
        if !is_in_path(GIO) {
            return;
        }
        let Ok(output) = execute_and_output(GIO, [MOUNT, "-li"]) else {
            return;
        };
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return;
        };

        content.extend(
            stdout
                .lines()
                .filter(|line| line.contains("activation_root"))
                .map(Mtp::from_gio)
                .filter_map(std::result::Result::ok)
                .map(Mountable::MTP),
        )
    }
}

/// Holds the mountable devices.
#[derive(Debug, Default)]
pub struct Mount {
    pub content: Vec<Mountable>,
    index: usize,
}

impl Mount {
    const WIDTHS: [Constraint; 5] = [
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Max(28),
        Constraint::Length(10),
        Constraint::Min(1),
    ];

    pub fn update(&mut self, disks: &Disks) -> Result<()> {
        self.index = 0;

        self.content = MountBuilder::build_from_json()?;
        MountBuilder::extend_with_remote(&mut self.content, disks);
        MountBuilder::extend_with_mtp_from_gio(&mut self.content);
        MountBuilder::extend_with_network(&mut self.content)?;

        #[cfg(debug_assertions)]
        log_info!("{self:#?}");
        Ok(())
    }

    pub fn umount_selected_no_password(&mut self) -> Result<bool> {
        match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount_no_password(),
            Mountable::Encrypted(_device) => {
                unreachable!("Encrypted devices can't be unmounted without password.")
            }
            Mountable::MTP(device) => device.umount(),
            Mountable::Network(_device) => Ok(false),
            Mountable::Remote(RemoteDevice {
                name: _,
                mountpoint,
            }) => umount_remote_no_password(mountpoint),
        }
    }

    pub fn selected_mount_point(&self) -> Option<PathBuf> {
        Some(PathBuf::from(self.selected()?.mountpoint()?))
    }

    pub fn mount_selected_no_password(&mut self) -> Result<bool> {
        match &mut self.content[self.index] {
            Mountable::Device(device) => device.mount_no_password(),
            Mountable::Encrypted(_device) => {
                unreachable!("Encrypted devices can't be mounted without password.")
            }
            Mountable::MTP(device) => device.mount(),
            Mountable::Network(_) => Ok(false),
            Mountable::Remote(_) => Ok(false),
        }
    }

    /// Open and mount the selected device.
    pub fn mount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<bool> {
        let success = match &mut self.content[self.index] {
            Mountable::Device(device) => device.mount(&current_username()?, password_holder)?,
            Mountable::Encrypted(_device) => {
                unreachable!("EncryptedBlockDevice should impl its own method")
            }
            Mountable::MTP(device) => device.mount()?,
            Mountable::Network(_) => false,
            Mountable::Remote(_) => false,
        };

        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(success)
    }

    pub fn umount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<()> {
        let username = current_username()?;
        let success = match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount(&username, password_holder)?,
            Mountable::MTP(device) => device.umount()?,
            Mountable::Network(device) => device.umount(password_holder)?,
            Mountable::Encrypted(_device) => {
                unreachable!("EncryptedBlockDevice should impl its own method")
            }
            Mountable::Remote(RemoteDevice {
                name: _,
                mountpoint,
            }) => umount_remote(mountpoint, password_holder)?,
        };
        if !success {
            reset_sudo_faillock()?
        }
        password_holder.reset();
        drop_sudo_privileges()?;
        Ok(())
    }

    pub fn eject_removable_device(&self) -> Result<bool> {
        let Some(Mountable::Device(device)) = &self.selected() else {
            return Ok(false);
        };
        device.try_power_off()
    }

    /// We receive the uuid of the _parent_ and must compare it to the parent of the device.
    /// Returns the mountpoint of the found device, if any.
    pub fn find_encrypted_by_uuid(&self, parent_uuid: Option<String>) -> Option<String> {
        for device in self.content() {
            let Mountable::Encrypted(device) = device else {
                continue;
            };
            if device.parent == parent_uuid && device.is_mounted() {
                return device.mountpoint.clone();
            }
        }
        None
    }

    fn header() -> Row<'static> {
        let header_style = MENU_STYLES
            .get()
            .expect("Menu colors should be set")
            .palette_4
            .fg
            .unwrap_or(Color::Rgb(0, 0, 0));
        Row::new([
            Cell::from(""),
            Cell::from("sym"),
            Cell::from("path"),
            Cell::from("label"),
            Cell::from("mountpoint"),
        ])
        .style(header_style)
    }

    fn row<'a>(&self, index: usize, item: &'a Mountable, style: Style) -> Row<'a> {
        let bind = Cell::from(format!("{bind:2<}", bind = index + 1));
        let symbols = Cell::from(Text::from(item.symbols()));
        let path = Cell::from(Text::from(item.path_repr()));
        let label = Cell::from(Text::from(item.label()));
        let mountpoint = Cell::from(Text::from(item.mountpoint_repr()));
        Row::new([bind, symbols, path, label, mountpoint]).style(self.style(index, &style))
    }
}

fn umount_remote(mountpoint: &str, password_holder: &mut PasswordHolder) -> Result<bool> {
    let success = set_sudo_session(password_holder)?;
    password_holder.reset();
    if !success {
        return Ok(false);
    }
    let (success, stdout, stderr) = execute_sudo_command_passwordless(&[UMOUNT, mountpoint])?;
    if !success {
        log_info!(
            "umount remote failed:\nstdout: {}\nstderr: {}",
            stdout,
            stderr
        );
    }

    Ok(success)
}

fn umount_remote_no_password(mountpoint: &str) -> Result<bool> {
    let output = execute_and_output(UMOUNT, [mountpoint])?;
    let success = output.status.success();
    if !success {
        log_info!(
            "umount {mountpoint}:\nstdout: {stdout}\nstderr: {stderr}",
            stdout = String::from_utf8(output.stdout)?,
            stderr = String::from_utf8(output.stderr)?,
        );
    }
    Ok(success)
}

/// True iff `lsblk` and `udisksctl` are in path.
/// Nothing here can be done without those programs.
pub fn lsblk_and_udisksctl_installed() -> bool {
    is_in_path(LSBLK) && is_in_path(UDISKSCTL)
}

fn get_devices_json() -> Result<String> {
    Ok(String::from_utf8(
        execute_and_output(
            LSBLK,
            [
                "--json",
                "-o",
                "FSTYPE,PATH,UUID,MOUNTPOINT,NAME,LABEL,HOTPLUG,MODEL",
            ],
        )?
        .stdout,
    )?)
}

fn truncate_string<S: AsRef<str>>(input: S, max_length: usize) -> String {
    if input.as_ref().chars().count() > max_length {
        let truncated: String = input.as_ref().chars().take(max_length).collect();
        format!("{}...", truncated)
    } else {
        input.as_ref().to_string()
    }
}
impl_content!(Mount, Mountable);

impl DrawMenu<Mountable> for Mount {
    fn draw_menu(&self, f: &mut Frame, rect: &Rect, window: &ContentWindow) {
        let mut p_rect = rect.offseted(2, 3);
        p_rect.height = p_rect.height.saturating_sub(2);
        p_rect.width = p_rect.width.saturating_sub(2);

        let content = self.content();
        let table = Table::new(
            colored_skip_take!(content, window)
                .map(|(index, item, style)| self.row(index, item, style)),
            Self::WIDTHS,
        )
        .header(Self::header());
        f.render_widget(table, p_rect);
    }
}
