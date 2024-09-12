use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::anyhow;
use anyhow::Result;
use syntect::highlighting::Theme;
use tuikit::attr::Color;

use crate::common::tilde;
use crate::common::CONFIG_PATH;
use crate::config::configuration::load_color_from_config;
use crate::config::Colorer;
use crate::config::Colors;
use crate::config::MenuColors;

/// Colors read from the config file.
/// We define a colors for every kind of file except normal files.
/// Colors for normal files are calculated from their extension and
/// are greens or blues.
///
/// Colors are setup on start and never change afterwards.
pub static COLORS: OnceLock<Colors> = OnceLock::new();

/// Defines a palette which will color the "normal" files based on their extension.
/// We try to read a yaml value and pick one of 3 palettes :
/// "green-red", "blue-green", "blue-red", "red-green", "red-blue", "green-blue" which is the default.
/// "custom" will create a gradient from start_palette to end_palette. Both values should be "rgb(u8, u8, u8)".
pub static COLORER: OnceLock<fn(usize) -> Color> = OnceLock::new();

/// First color of palette for normal file
pub static START_COLOR: OnceLock<(u8, u8, u8)> = OnceLock::new();

/// Last color of palette for normal file
pub static STOP_COLOR: OnceLock<(u8, u8, u8)> = OnceLock::new();

/// Menu color struct
pub static MENU_COLORS: OnceLock<MenuColors> = OnceLock::new();

/// Highlighting theme color used to preview code file
pub static MONOKAI_THEME: OnceLock<Theme> = OnceLock::new();

/// Starting folder of the application. Read from arguments if any `-P ~/Downloads` else it uses the current folder: `.`.
pub static START_FOLDER: OnceLock<PathBuf> = OnceLock::new();

fn set_start_folder(start_folder: &str) -> Result<()> {
    START_FOLDER
        .set(std::fs::canonicalize(tilde(start_folder).as_ref()).unwrap_or_default())
        .map_err(|_| anyhow!("Start folder shouldn't be set"))?;
    Ok(())
}

fn set_menu_colors() -> Result<()> {
    MENU_COLORS
        .set(MenuColors::default().update())
        .map_err(|_| anyhow!("Menu colors shouldn't be set"))?;
    Ok(())
}

fn set_start_stop_colors() -> Result<()> {
    let start_color = load_color_from_config("start").unwrap_or((40, 40, 40));
    let stop_color = load_color_from_config("stop").unwrap_or((180, 180, 180));

    START_COLOR
        .set(start_color)
        .map_err(|_| anyhow!("Start color shouldn't be set"))?;
    STOP_COLOR
        .set(stop_color)
        .map_err(|_| anyhow!("Stop color shouldn't be set"))?;
    Ok(())
}

fn set_file_colors() -> Result<()> {
    let mut colors = Colors::default();
    if let Ok(file) = File::open(Path::new(&tilde(CONFIG_PATH).to_string())) {
        if let Ok(yaml) = serde_yaml::from_reader::<File, serde_yaml::value::Value>(file) {
            colors.update_from_config(&yaml["colors"]);
        };
    };
    COLORS
        .set(colors)
        .map_err(|_| anyhow!("File colors shouldn't be set"))?;
    Ok(())
}

fn read_colorer() -> fn(usize) -> Color {
    let colorer = Colorer::color_green_blue as fn(usize) -> Color;
    let Ok(file) = File::open(std::path::Path::new(&tilde(CONFIG_PATH).to_string())) else {
        return colorer;
    };
    let Ok(yaml) = serde_yaml::from_reader::<File, serde_yaml::value::Value>(file) else {
        return colorer;
    };
    let Some(start) = yaml["palette"]["start"].as_str() else {
        return colorer;
    };
    let Some(stop) = yaml["palette"]["stop"].as_str() else {
        return colorer;
    };
    match (start.to_owned() + "-" + stop).as_ref() {
        "green-blue" => Colorer::color_green_blue as fn(usize) -> Color,
        "red-blue" => Colorer::color_red_blue as fn(usize) -> Color,
        "red-green" => Colorer::color_red_green as fn(usize) -> Color,
        "blue-green" => Colorer::color_blue_green as fn(usize) -> Color,
        "blue-red" => Colorer::color_blue_red as fn(usize) -> Color,
        "green-red" => Colorer::color_green_red as fn(usize) -> Color,
        _ => Colorer::color_custom as fn(usize) -> Color,
    }
}

fn set_colorer() -> Result<()> {
    COLORER
        .set(read_colorer())
        .map_err(|_| anyhow!("Colorer shouldn't be set"))?;
    Ok(())
}

/// Set all the values which could be configured from config file or arguments staticly.
/// It allows us to read those values globally without having to pass them through to every function.
/// All values use a [`std::sync::OnceLock`] internally.
pub fn set_configurable_static(start_folder: &str) -> Result<()> {
    set_menu_colors()?;
    set_start_stop_colors()?;
    set_file_colors()?;
    set_colorer()?;
    set_start_folder(start_folder)
}
