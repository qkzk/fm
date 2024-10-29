use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::anyhow;
use anyhow::Result;
use ratatui::style::Color;
use syntect::highlighting::Theme;

use crate::common::tilde;
use crate::config::{
    read_normal_file_colorer, FileStyle, Gradient, MenuStyle, NormalFileColorer,
    MAX_GRADIENT_NORMAL,
};

/// Starting folder of the application. Read from arguments if any `-P ~/Downloads` else it uses the current folder: `.`.
pub static START_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Colors read from the config file.
/// We define a colors for every kind of file except normal files.
/// Colors for normal files are calculated from their extension and
/// are greens or blues.
///
/// Colors are setup on start and never change afterwards.
pub static FILE_STYLES: OnceLock<FileStyle> = OnceLock::new();

/// Menu color struct
pub static MENU_STYLES: OnceLock<MenuStyle> = OnceLock::new();

/// Defines a palette which will color the "normal" files based on their extension.
/// We try to read a yaml value and pick one of 3 palettes :
/// "green-red", "blue-green", "blue-red", "red-green", "red-blue", "green-blue" which is the default.
/// "custom" will create a gradient from start_palette to end_palette. Both values should be "rgb(u8, u8, u8)".
pub static COLORER: OnceLock<fn(usize) -> Color> = OnceLock::new();

/// Gradient for normal files
pub static ARRAY_GRADIENT: OnceLock<[Color; MAX_GRADIENT_NORMAL]> = OnceLock::new();

/// Highlighting theme color used to preview code file
pub static MONOKAI_THEME: OnceLock<Theme> = OnceLock::new();

fn set_start_folder(start_folder: &str) -> Result<()> {
    START_FOLDER
        .set(std::fs::canonicalize(tilde(start_folder).as_ref()).unwrap_or_default())
        .map_err(|_| anyhow!("Start folder shouldn't be set"))?;
    Ok(())
}

fn set_file_styles() -> Result<()> {
    FILE_STYLES
        .set(FileStyle::from_config())
        .map_err(|_| anyhow!("File colors shouldn't be set"))?;
    Ok(())
}

fn set_menu_styles() -> Result<()> {
    MENU_STYLES
        .set(MenuStyle::default().update())
        .map_err(|_| anyhow!("Menu colors shouldn't be set"))?;
    Ok(())
}

fn set_normal_file_colorer() -> Result<()> {
    let (start_color, stop_color) = read_normal_file_colorer();
    ARRAY_GRADIENT
        .set(Gradient::new(start_color, stop_color, MAX_GRADIENT_NORMAL).as_array()?)
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
    set_menu_styles()?;
    set_file_styles()?;
    set_normal_file_colorer()
}
