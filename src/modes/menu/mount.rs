use std::{
    borrow::Cow,
    fmt::Display,
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{from_str, from_value, Value};
use sysinfo::Disks;

use crate::common::{
    current_uid, current_username, is_dir_empty, is_in_path, CRYPTSETUP, GIO, LSBLK, MKDIR, MOUNT,
    UDISKSCTL, UMOUNT,
};
use crate::io::{
    drop_sudo_privileges, execute_and_output, execute_sudo_command,
    execute_sudo_command_with_password, reset_sudo_faillock, set_sudo_session, CowStr, DrawMenu,
};
use crate::modes::{MountCommands, MountParameters, MountRepr, PasswordHolder};
use crate::{impl_content, impl_selectable, log_info, log_line};

/// Possible actions on encrypted drives
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockDeviceAction {
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

#[derive(Debug)]
pub struct NetworkMount {
    pub kind: NetworkKind,
    pub path: String,
    pub mountpoint: String,
}

/// Holds a network mount point.
/// Parsed from a line of /proc/self/mountinfo
/// 96 29 0:60 / /home/user/nfs rw,relatime shared:523 - nfs4 hostname:/remote/path rw,vers=4.2,rsize=524288,wsize=524288,namlen=255,hard,proto=tcp,timeo=900,retrans=5,sec=sys,clientaddr=192.168.1.17,local_lock=none,addr=remote_ip
/// 483 29 0:73 / /home/user/cifs rw,relatime shared:424 - cifs //ip_adder/qnas rw,vers=3.1.1,cache=strict,username=quentin,uid=0,noforceuid,gid=0,noforcegid,addr=yout_ip,file_mode=0755,dir_mode=0755,soft,nounix,serverino,mapposix,reparse=nfs,rsize=4194304,wsize=4194304,bsize=1048576,retrans=1,echo_interval=60,actimeo=1,closetimeo=1
impl NetworkMount {
    fn umount(&self) -> Result<bool> {
        let success = execute_and_output(UMOUNT, [self.mountpoint.as_str()])?
            .status
            .success();

        log_info!(
            "Unmounted {device}. Success ? {success}",
            device = self.mountpoint,
        );
        Ok(success)
    }
}

impl MountRepr for NetworkMount {
    fn as_string(&self) -> Result<String> {
        Ok(format!(
            "MN {kind} {path} -> {mountpoint}",
            kind = self.kind,
            path = self.path,
            mountpoint = self.mountpoint
        ))
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
}

impl MountRepr for Mtp {
    /// String representation of the device
    fn as_string(&self) -> Result<String> {
        let is_mounted = self.is_mounted();
        let mut repr = format!(
            "{mount_repr}P {name}",
            mount_repr = if is_mounted { "M" } else { "U" },
            name = self.name.clone()
        );
        if is_mounted {
            repr.push_str(" -> ");
            repr.push_str(&self.path)
        }

        Ok(repr)
    }
}

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
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            MKDIR.to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.uuid.clone().unwrap()),
        ]
    }

    fn format_mount_parameters(&self, username: &str) -> Vec<String> {
        vec![
            MOUNT.to_owned(),
            format!("/dev/mapper/{}", self.uuid.clone().unwrap()),
            format!("/run/media/{}/{}", username, self.uuid.clone().unwrap()),
        ]
    }

    fn format_umount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            UDISKSCTL.to_owned(),
            "unmount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }
}

impl MountRepr for EncryptedBlockDevice {
    /// String representation of the device.
    fn as_string(&self) -> Result<String> {
        let mut repr = format!(
            "{is_mounted}C {path} {label}",
            is_mounted = if self.is_mounted() { "M" } else { "U" },
            label = self.label_repr(),
            path = truncate_string(&self.path, 20)
        );
        if let Some(mountpoint) = &self.mountpoint {
            repr.push_str(" -> ");
            repr.push_str(&truncate_string(mountpoint, 25));
        }
        Ok(repr)
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

    pub fn open_mount_crypto(
        &self,
        username: &str,
        password_holder: &mut PasswordHolder,
    ) -> Result<bool> {
        let success = is_in_path(CRYPTSETUP)
            && self.set_sudo_session(password_holder)?
            && self.execute_luks_open(password_holder)?
            && self.execute_mkdir_crypto(username)?
            && self.execute_mount_crypto(username)?;
        drop_sudo_privileges()?;
        Ok(success)
    }

    fn set_sudo_session(&self, password_holder: &mut PasswordHolder) -> Result<bool> {
        if !set_sudo_session(password_holder)? {
            password_holder.reset();
            return Ok(false);
        }
        Ok(true)
    }

    fn execute_luks_open(&self, password_holder: &mut PasswordHolder) -> Result<bool> {
        let (success, stdout, stderr) = execute_sudo_command_with_password(
            &self.format_luksopen_parameters(),
            password_holder
                .cryptsetup()
                .as_ref()
                .context("cryptsetup password_holder isn't set")?,
            std::path::Path::new("/"),
        )?;
        password_holder.reset();
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    fn execute_mkdir_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_mkdir_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        Ok(success)
    }

    fn execute_mount_crypto(&self, username: &str) -> Result<bool> {
        let (success, stdout, stderr) =
            execute_sudo_command(&self.format_mount_parameters(username))?;
        log_info!("stdout: {}\nstderr: {}", stdout, stderr);
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
            execute_sudo_command(&self.format_umount_parameters(username))?;
        if !success {
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        }
        Ok(success)
    }

    fn execute_luks_close(&self) -> Result<bool> {
        let (success, stdout, stderr) = execute_sudo_command(&self.format_luksclose_parameters())?;
        if !success {
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
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
}

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
        let mut args = self.format_mount_parameters("");
        let output = execute_and_output(&args.remove(0), &args)?;
        Ok(output.status.success())
    }

    fn umount_no_password(&self) -> Result<bool> {
        let mut args = self.format_umount_parameters("");
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

    pub fn power_off(&self) -> Result<bool> {
        if !self.hotplug && !self.is_mounted() {
            return Ok(false);
        }
        let output = execute_and_output(UDISKSCTL, ["power-off", "-b", &self.path])?;
        Ok(output.status.success())
    }
}

impl MountParameters for BlockDevice {
    fn format_mkdir_parameters(&self, username: &str) -> [String; 3] {
        [
            MKDIR.to_owned(),
            "-p".to_owned(),
            format!("/run/media/{}/{}", username, self.device_name()),
        ]
    }

    fn format_mount_parameters(&self, _username: &str) -> Vec<String> {
        vec![
            UDISKSCTL.to_owned(),
            "mount".to_owned(),
            "--block-device".to_owned(),
            self.path.to_owned(),
        ]
    }

    fn format_umount_parameters(&self, _username: &str) -> Vec<String> {
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
        let args_sudo = self.format_mount_parameters(username);
        let (success, stdout, stderr) = execute_sudo_command(&args_sudo)?;
        if !success {
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
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
            execute_sudo_command(&self.format_umount_parameters(username))?;
        if !success {
            log_info!("stdout: {}\nstderr: {}", stdout, stderr);
        }
        Ok(success)
    }
}

impl MountRepr for BlockDevice {
    /// String representation of the device.
    fn as_string(&self) -> Result<String> {
        let mut repr = format!(
            "{is_mounted}{prefix} {path} {label}",
            is_mounted = if self.is_mounted() { "M" } else { "U" },
            prefix = self.prefix_repr(),
            label = self.label_repr(),
            path = self.path
        );
        if let Some(mountpoint) = &self.mountpoint {
            repr.push_str(" -> ");
            repr.push_str(mountpoint)
        }
        Ok(repr)
    }
}

#[derive(Debug)]
pub enum Mountable {
    Device(BlockDevice),
    Encrypted(EncryptedBlockDevice),
    MTP(Mtp),
    Remote((String, String)),
    Network(NetworkMount),
}

impl Mountable {
    fn as_string(&self) -> Result<String> {
        match &self {
            Self::Device(device) => device.as_string(),
            Self::Encrypted(device) => device.as_string(),
            Self::MTP(device) => device.as_string(),
            Self::Network(device) => device.as_string(),
            Self::Remote((remote_desc, local_path)) => {
                Ok(format!("MS {remote_desc} -> {local_path}"))
            }
        }
    }

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
            Self::Remote(_) => true,
        }
    }

    pub fn path(&self) -> &str {
        match &self {
            Self::Device(device) => device.path.as_str(),
            Self::Encrypted(device) => device.path.as_str(),
            Self::MTP(device) => device.path.as_str(),
            Self::Network(device) => device.path.as_str(),
            Self::Remote((_, local_path)) => local_path.as_str(),
        }
    }

    fn mountpoint(&self) -> Option<&str> {
        match self {
            Mountable::Device(device) => device.mountpoint.as_deref(),
            Mountable::Encrypted(device) => device.mountpoint.as_deref(),
            Mountable::MTP(device) => Some(&device.path),
            Mountable::Network(device) => Some(&device.mountpoint),
            Mountable::Remote((_name, mountpoint)) => Some(mountpoint),
        }
    }
}

impl CowStr for Mountable {
    fn cow_str(&self) -> Cow<str> {
        self.as_string().unwrap_or_default().into()
    }
}

#[derive(Default, Debug)]
pub struct Mount {
    pub content: Vec<Mountable>,
    index: usize,
}

impl Mount {
    pub fn update(&mut self, disks: &Disks) -> Result<()> {
        self.index = 0;

        self.content = Self::build_from_json()?;
        self.extend_with_remote(disks);
        self.extend_with_mtp_from_gio();
        self.extend_with_network()?;

        #[cfg(debug_assertions)]
        log_info!("{self:#?}");
        Ok(())
    }

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

    fn extend_with_remote(&mut self, disks: &Disks) {
        self.content.extend(
            disks
                .iter()
                .filter(|d| d.file_system().to_string_lossy().contains("sshfs"))
                .map(|d| {
                    Mountable::Remote((
                        d.name().to_string_lossy().to_string(),
                        d.mount_point().to_string_lossy().to_string(),
                    ))
                })
                .collect::<Vec<_>>(),
        );
    }

    fn extend_with_network(&mut self) -> Result<()> {
        self.content.extend(Self::get_network_devices()?);
        Ok(())
    }

    fn get_network_devices() -> io::Result<Vec<Mountable>> {
        let file = File::open("/proc/self/mountinfo")?;
        let reader = BufReader::new(file);
        let mut network_mountables = vec![];

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() <= 6 {
                continue;
            }
            let Some(fstype) = parts.get(parts.len() - 3) else {
                continue;
            };
            if *fstype == "cifs" || *fstype == "nfs4" {
                let Some(kind) = NetworkKind::from_fs_type(fstype) else {
                    continue;
                };
                let mountpoint = parts.get(4).unwrap_or(&"").to_string();
                let path = parts.get(parts.len() - 2).unwrap_or(&"").to_string();
                if path.is_empty() || mountpoint.is_empty() {
                    continue;
                }
                network_mountables.push(Mountable::Network(NetworkMount {
                    kind,
                    mountpoint,
                    path,
                }))
            }
        }
        Ok(network_mountables)
    }

    fn extend_with_mtp_from_gio(&mut self) {
        if !is_in_path(GIO) {
            return;
        }
        let Ok(output) = execute_and_output(GIO, [MOUNT, "-li"]) else {
            return;
        };
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return;
        };

        self.content.extend(
            stdout
                .lines()
                .filter(|line| line.contains("activation_root"))
                .map(Mtp::from_gio)
                .filter_map(std::result::Result::ok)
                .map(Mountable::MTP),
        )
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

    pub fn umount_selected_no_password(&mut self) -> Result<bool> {
        match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount_no_password(),
            Mountable::Encrypted(_device) => {
                unreachable!("Encrypted devices can't be unmounted without password.")
            }
            Mountable::MTP(device) => device.umount(),
            Mountable::Network(device) => device.umount(),
            Mountable::Remote((_name, mountpoint)) => umount_remote_no_password(mountpoint),
        }
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

    pub fn selected_mount_point(&self) -> Option<PathBuf> {
        Some(PathBuf::from(self.selected()?.mountpoint()?))
    }

    pub fn umount_selected(&mut self, password_holder: &mut PasswordHolder) -> Result<()> {
        let username = current_username()?;
        let success = match &mut self.content[self.index] {
            Mountable::Device(device) => device.umount(&username, password_holder)?,
            Mountable::MTP(device) => device.umount()?,
            Mountable::Network(device) => device.umount()?,
            Mountable::Encrypted(_device) => {
                unreachable!("EncryptedBlockDevice should impl its own method")
            }
            Mountable::Remote((_, mountpoint)) => umount_remote(mountpoint, password_holder)?,
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
        device.power_off()
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
}

fn umount_remote(mountpoint: &str, password_holder: &mut PasswordHolder) -> Result<bool> {
    let success = set_sudo_session(password_holder)?;
    password_holder.reset();
    if !success {
        return Ok(false);
    }
    let (success, stdout, stderr) = execute_sudo_command(&[UMOUNT, mountpoint])?;
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

/// True iff `lsblk` and `cryptsetup` are in path.
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
impl_selectable!(Mount);
impl_content!(Mount, Mountable);
impl DrawMenu<Mountable> for Mount {}
