use anyhow::{Context, Result};

use crate::common::{tilde, CONFIG_FOLDER};

const DEFAULT_CONFIG: &str = include_str!("../../config_files/fm/config.yaml");
const DEFAULT_CLI: &str = include_str!("../../config_files/fm/cli.yaml");
const DEFAULT_OPENER: &str = include_str!("../../config_files/fm/opener.yaml");
const DEFAULT_SESSION: &str = include_str!("../../config_files/fm/session.yaml");
const DEFAULT_TUIS: &str = include_str!("../../config_files/fm/tuis.yaml");
const DEFAULT_LOG_FM: &str = include_str!("../../config_files/fm/log/fm.log");
const DEFAULT_LOG_ACTION_LOGGER: &str = include_str!("../../config_files/fm/log/action_logger.log");
const DEFAULT_LOG_INPUT_HISTORY: &str = include_str!("../../config_files/fm/log/input_history.log");

const DEFAULT_CONFIGS: [(&str, &str); 8] = [
    ("config.yaml", DEFAULT_CONFIG),
    ("cli.yaml", DEFAULT_CLI),
    ("opener.yaml", DEFAULT_OPENER),
    ("session.yaml", DEFAULT_SESSION),
    ("tuis.yaml", DEFAULT_TUIS),
    ("log/fm.log", DEFAULT_LOG_FM),
    ("log/action_logger.log", DEFAULT_LOG_ACTION_LOGGER),
    ("log/input_history.log", DEFAULT_LOG_INPUT_HISTORY),
];

const TRASH_FOLDERS: [&str; 3] = [
    "~/.local/share/Trash/expunged",
    "~/.local/share/Trash/files",
    "~/.local/share/Trash/info",
];

/// Creates the default config if it doesn't exists.
/// Creates the trash folder if it doesn't exists.
///
/// Errors
///
/// It may fail if the user has no write access to $HOME which shouldn't happen in a normal environment.
pub fn make_default_config_files() -> Result<()> {
    create_config_folder()?;
    copy_default_config_files()?;
    create_trash_folders()?;
    Ok(())
}

/// Creates the config folder in ~/.config/fm
fn create_config_folder() -> std::io::Result<()> {
    let p = tilde(CONFIG_FOLDER);
    std::fs::create_dir_all(p.as_ref())
}

/// Ensure a file content by creating its parent folder and writing the content
/// to the path.
fn ensure_config(path: &std::path::Path, contents: &str) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path.parent().context("No parent")?)?;
        std::fs::write(path, contents)?;
    }
    Ok(())
}

/// Copy the config files to ~/.config/fm/
fn copy_default_config_files() -> Result<()> {
    let dest = std::path::PathBuf::from(tilde(CONFIG_FOLDER).as_ref());

    for (config_rel_path, contents) in &DEFAULT_CONFIGS {
        let mut path = dest.clone();
        path.push(config_rel_path);
        ensure_config(&path, contents)?;
    }
    Ok(())
}

/// Creates the trash folders:
///  ~.local
///     |- Trash
///          |- expunged/
///          |- files/
///          |- info/
fn create_trash_folders() -> std::io::Result<()> {
    for dir in &TRASH_FOLDERS {
        std::fs::create_dir_all(tilde(dir).as_ref())?;
    }
    Ok(())
}
