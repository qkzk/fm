use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::anyhow;
use anyhow::Result;
use serde_yml::from_reader;
use serde_yml::Value;
use syntect::highlighting::Theme;
use tuikit::attr::Color;

use crate::common::tilde;
use crate::common::CONFIG_PATH;
use crate::config::FileAttr;
use crate::config::MenuAttrs;
use crate::config::NormalFileColorer;

use super::{read_normal_file_colorer, Gradient};

/// Starting folder of the application. Read from arguments if any `-P ~/Downloads` else it uses the current folder: `.`.
pub static START_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Colors read from the config file.
/// We define a colors for every kind of file except normal files.
/// Colors for normal files are calculated from their extension and
/// are greens or blues.
///
/// Colors are setup on start and never change afterwards.
pub static FILE_ATTRS: OnceLock<FileAttr> = OnceLock::new();

/// Menu color struct
pub static MENU_ATTRS: OnceLock<MenuAttrs> = OnceLock::new();

/// Defines a palette which will color the "normal" files based on their extension.
/// We try to read a yaml value and pick one of 3 palettes :
/// "green-red", "blue-green", "blue-red", "red-green", "red-blue", "green-blue" which is the default.
/// "custom" will create a gradient from start_palette to end_palette. Both values should be "rgb(u8, u8, u8)".
pub static COLORER: OnceLock<fn(usize) -> Color> = OnceLock::new();

/// Gradient for normal files
pub static ARRAY_GRADIENT: OnceLock<[Color; 254]> = OnceLock::new();

/// Highlighting theme color used to preview code file
pub static MONOKAI_THEME: OnceLock<Theme> = OnceLock::new();

fn set_start_folder(start_folder: &str) -> Result<()> {
    START_FOLDER
        .set(std::fs::canonicalize(tilde(start_folder).as_ref()).unwrap_or_default())
        .map_err(|_| anyhow!("Start folder shouldn't be set"))?;
    Ok(())
}

fn set_file_attrs() -> Result<()> {
    let mut attrs = FileAttr::default();
    if let Ok(file) = File::open(Path::new(&tilde(CONFIG_PATH).to_string())) {
        if let Ok(yaml) = from_reader::<File, Value>(file) {
            attrs.update_from_config(&yaml["colors"]);
        };
    };
    FILE_ATTRS
        .set(attrs)
        .map_err(|_| anyhow!("File colors shouldn't be set"))?;
    Ok(())
}

fn set_menu_attrs() -> Result<()> {
    MENU_ATTRS
        .set(MenuAttrs::default().update())
        .map_err(|_| anyhow!("Menu colors shouldn't be set"))?;
    Ok(())
}

fn set_normal_file_colorer() -> Result<()> {
    let (start_color, stop_color) = read_normal_file_colorer();
    ARRAY_GRADIENT
        .set(Gradient::new(start_color, stop_color, 254).into_array()?)
        .map_err(|_| anyhow!("Gradient shouldn't be set"))?;
    COLORER
        .set(NormalFileColorer::colorer as fn(usize) -> Color)
        .map_err(|_| anyhow!("Colorer shouldn't be set"))?;

    Ok(())
}

/// Set all the values which could be configured from config file or arguments staticly.
/// It allows us to read those values globally without having to pass them through to every function.
/// All values use a [`std::sync::OnceLock`] internally.
pub fn set_configurable_static(start_folder: &str) -> Result<()> {
    set_start_folder(start_folder)?;
    set_menu_attrs()?;
    set_file_attrs()?;
    set_normal_file_colorer()
}
