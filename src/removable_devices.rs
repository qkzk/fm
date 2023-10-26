use anyhow::{anyhow, Result};
use log::info;

use crate::impl_selectable_content;

#[derive(Debug, Clone, Default)]
pub struct RemovableDevices {
    pub content: Vec<Removable>,
    pub index: usize,
}

impl RemovableDevices {
    fn from_gio() -> Option<Self> {
        let Ok(output) = std::process::Command::new("gio").args(["-li"]).output() else {
            return None;
        };
        let Ok(stdout) = String::from_utf8(output.stdout) else {
            return None;
        };

        let content: Vec<_> = stdout
            .lines()
            .filter(|line| line.contains("activation_root"))
            .map(|line| line.to_owned())
            .map(|line| Removable::from_gio(line))
            .filter_map(|removable| removable.ok())
            .collect();

        if content.is_empty() {
            None
        } else {
            Some(Self { content, index: 0 })
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Removable {
    pub device_name: String,
    pub path: String,
    is_mounted: bool,
}

impl Removable {
    fn from_gio(line: String) -> Result<Self> {
        let device_name = line.replace("mtp:://", "");
        let uid = users::get_current_uid();
        let path = format!("/run/user/{uid}/gvfs/{line}");
        let pb_path = std::path::Path::new(&path);
        let is_mounted = pb_path.exists() && pb_path.read_dir()?.next().is_some();
        Ok(Self {
            device_name,
            path,
            is_mounted,
        })
    }
}

impl_selectable_content!(Removable, RemovableDevices);
