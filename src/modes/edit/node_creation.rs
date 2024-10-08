use std::fmt::Display;
use std::fs;

use anyhow::{Context, Result};

use crate::app::{Status, Tab};
use crate::log_line;
use crate::modes::Display as DisplayMode;

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
    /// Create a new file or directory in current dir.
    /// The filename is read from inputstring.
    ///
    /// # Errors
    ///
    /// It may fail if the node creation fail. See [`std::fs::create_dir_all`] and [`std::fs::File::create`]
    pub fn create(&self, status: &mut Status) -> Result<std::path::PathBuf> {
        let tab = status.current_tab_mut();
        let root_path = Self::root_path(tab)?;
        let path = root_path.join(status.menu.input.string());

        if path.exists() {
            log_line!("{self} {path} already exists", path = path.display());
            return Err(anyhow::anyhow!(
                "File {path} alredy exists",
                path = path.display()
            ));
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
        Ok(path)
    }

    fn root_path(tab: &mut Tab) -> Result<std::path::PathBuf> {
        let root_path = match tab.display_mode {
            DisplayMode::Tree => tab
                .tree
                .directory_of_selected()
                .context("no parent")?
                .to_owned(),
            _ => tab.directory.path.to_path_buf(),
        };
        Ok(root_path)
    }
}
