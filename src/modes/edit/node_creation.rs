use std::fmt::Display;
use std::fs;

use anyhow::{Context, Result};

use crate::app::Tab;
use crate::log_line;
use crate::modes::DisplayMode;

/// Used to create of files or directory.
pub enum NodeCreation {
    Newfile,
    Newdir,
}

impl Display for NodeCreation {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Newfile => write!(f, "file"),
            Self::Newdir => write!(f, "directory"),
        }
    }
}

impl NodeCreation {
    pub fn create(&self, tab: &mut Tab) -> Result<()> {
        let root_path = Self::root_path(tab)?;
        let path = root_path.join(sanitize_filename::sanitize(tab.input.string()));

        if path.exists() {
            log_line!("{self} {path} already exists", path = path.display());
            return Ok(());
        };

        match self {
            Self::Newdir => {
                fs::create_dir_all(&path)?;
            }
            Self::Newfile => {
                fs::File::create(&path)?;
            }
        }
        log_line!("Created new {self}: {path}", path = path.display());
        Ok(())
    }

    fn root_path(tab: &mut Tab) -> Result<std::path::PathBuf> {
        let root_path = match tab.display_mode {
            DisplayMode::Tree => tab
                .tree
                .directory_of_selected()
                .context("no parent")?
                .to_owned(),
            _ => tab.path_content.path.clone(),
        };
        Ok(root_path)
    }
}