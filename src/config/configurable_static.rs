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
use crate::config::configuration::load_color_from_config;
use crate::config::FileAttr;
use crate::config::MenuAttrs;
use crate::config::NormalFileColorer;

use super::{ColorG, Gradient};

/// Colors read from the config file.
/// We define a colors for every kind of file except normal files.
/// Colors for normal files are calculated from their extension and
/// are greens or blues.
///
/// Colors are setup on start and never change afterwards.
pub static FILE_ATTRS: OnceLock<FileAttr> = OnceLock::new();

/// Defines a palette which will color the "normal" files based on their extension.
/// We try to read a yaml value and pick one of 3 palettes :
/// "green-red", "blue-green", "blue-red", "red-green", "red-blue", "green-blue" which is the default.
/// "custom" will create a gradient from start_palette to end_palette. Both values should be "rgb(u8, u8, u8)".
pub static COLORER: OnceLock<fn(usize) -> Color> = OnceLock::new();

/// Gradient for normal files
pub static GRADIENT_NORMAL_FILE: OnceLock<Gradient> = OnceLock::new();

/// Menu color struct
pub static MENU_ATTRS: OnceLock<MenuAttrs> = OnceLock::new();

/// Highlighting theme color used to preview code file
pub static MONOKAI_THEME: OnceLock<Theme> = OnceLock::new();

/// Starting folder of the application. Read from arguments if any `-P ~/Downloads` else it uses the current folder: `.`.
pub static START_FOLDER: OnceLock<PathBuf> = OnceLock::new();

pub static GREEN_BLUE: OnceLock<[Color; 254]> = OnceLock::new();

fn set_green_blue() -> Result<()> {
    let palette: Vec<Color> = (128..255)
        .map(|b| Color::Rgb(255, 0, b))
        .chain((128..255).map(|r| Color::Rgb(r, 0, 255)))
        .collect();
    let mut p = [Color::Rgb(0, 0, 0); 254];
    p.copy_from_slice(&palette);
    GREEN_BLUE.set(p).map_err(|_| anyhow!(""))?;
    Ok(())
}

fn set_gradient_normal_file() -> Result<()> {
    let start_color = load_color_from_config("start").unwrap_or((40, 40, 40));
    let stop_color = load_color_from_config("stop").unwrap_or((180, 180, 180));
    GRADIENT_NORMAL_FILE
        .set(Gradient::new(
            ColorG::new(start_color),
            ColorG::new(stop_color),
            255,
        ))
        .map_err(|_| anyhow!("GRADIENT_NORMAL_FILE shouldn't be set"))?;
    Ok(())
}

fn set_start_folder(start_folder: &str) -> Result<()> {
    START_FOLDER
        .set(std::fs::canonicalize(tilde(start_folder).as_ref()).unwrap_or_default())
        .map_err(|_| anyhow!("Start folder shouldn't be set"))?;
    Ok(())
}

fn set_menu_attrs() -> Result<()> {
    MENU_ATTRS
        .set(MenuAttrs::default().update())
        .map_err(|_| anyhow!("Menu colors shouldn't be set"))?;
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

fn read_colorer() -> fn(usize) -> Color {
    let colorer = NormalFileColorer::color_green_blue as fn(usize) -> Color;
    let Ok(file) = File::open(std::path::Path::new(&tilde(CONFIG_PATH).to_string())) else {
        return colorer;
    };
    let Ok(yaml) = from_reader::<File, Value>(file) else {
        return colorer;
    };
    let Some(start) = yaml["palette"]["start"].as_str() else {
        return colorer;
    };
    let Some(stop) = yaml["palette"]["stop"].as_str() else {
        return colorer;
    };
    match (start.to_owned() + "-" + stop).as_ref() {
        "green-blue" => NormalFileColorer::color_green_blue as fn(usize) -> Color,
        "red-blue" => NormalFileColorer::color_red_blue as fn(usize) -> Color,
        "red-green" => NormalFileColorer::color_red_green as fn(usize) -> Color,
        "blue-green" => NormalFileColorer::color_blue_green as fn(usize) -> Color,
        "blue-red" => NormalFileColorer::color_blue_red as fn(usize) -> Color,
        "green-red" => NormalFileColorer::color_green_red as fn(usize) -> Color,
        _ => NormalFileColorer::color_custom as fn(usize) -> Color,
    }
}

fn set_colorer() -> Result<()> {
    COLORER
        .set(read_colorer())
        .map_err(|_| anyhow!("Colorer shouldn't be set"))?;
    Ok(())
}

fn set_gradient_menu() -> Result<()> {
    Ok(())
}

/// Set all the values which could be configured from config file or arguments staticly.
/// It allows us to read those values globally without having to pass them through to every function.
/// All values use a [`std::sync::OnceLock`] internally.
pub fn set_configurable_static(start_folder: &str) -> Result<()> {
    set_menu_attrs()?;
    set_gradient_normal_file()?;
    set_file_attrs()?;
    set_colorer()?;
    set_gradient_menu()?;
    set_start_folder(start_folder)
}
