use std::{fs::File, path};

use anyhow::Result;
use ratatui::style::{Color, Style};
use serde_yml::{from_reader, Value};

use crate::common::{tilde, CONFIG_PATH, SYNTECT_DEFAULT_THEME};
use crate::config::{Bindings, ColorG};

/// Holds every configurable aspect of the application.
/// All styles are hardcoded then updated from optional values
/// of the config file.
/// The config file is a YAML file in `~/.config/fm/config.yaml`
#[derive(Debug, Clone)]
pub struct Config {
    /// Configurable keybindings.
    pub binds: Bindings,
}

impl Default for Config {
    /// Returns a default config with hardcoded values.
    fn default() -> Self {
        Self {
            binds: Bindings::default(),
        }
    }
}

impl Config {
    /// Updates the config from a yaml value read in the configuration file.
    fn update_from_config(&mut self, yaml: &Value) -> Result<()> {
        self.binds.update_normal(&yaml["keys"]);
        self.binds.update_custom(&yaml["custom"]);
        Ok(())
    }
}

/// Returns a config with values from :
///
/// 1. hardcoded values
///
/// 2. configured values from `~/.config/fm/config_file_name.yaml` if those files exists.
///
/// If the config file is poorly formated its simply ignored.
pub fn load_config(path: &str) -> Result<Config> {
    let mut config = Config::default();
    let Ok(file) = File::open(path::Path::new(&tilde(path).to_string())) else {
        crate::log_info!("Couldn't read config file at {path}");
        return Ok(config);
    };
    let Ok(yaml) = from_reader(file) else {
        return Ok(config);
    };
    let _ = config.update_from_config(&yaml);
    Ok(config)
}

/// Reads the config file and parse the "palette" values.
/// The palette format looks like this (with different accepted format)
/// ```yaml
/// colors:
///   normal_start: yellow, #ffff00, rgb(255, 255, 0)
///   normal_stop:  #ff00ff
/// ```
/// Recognized formats are : ansi names (yellow, light_red etc.), rgb like rgb(255, 55, 132) and hexadecimal like #ff3388.
/// The ANSI names are recognized but we can't get the user settings for all kinds of terminal
/// so we'll have to use default values.
///
/// If we can't read those values, we'll return green and blue.
pub fn read_normal_file_colorer() -> (ColorG, ColorG) {
    let default_pair = (ColorG::new(0, 255, 0), ColorG::new(0, 0, 255));
    let Ok(file) = File::open(tilde(CONFIG_PATH).as_ref()) else {
        return default_pair;
    };
    let Ok(yaml) = from_reader::<File, Value>(file) else {
        return default_pair;
    };
    let Some(start) = yaml["colors"]["normal_start"].as_str() else {
        return default_pair;
    };
    let Some(stop) = yaml["colors"]["normal_stop"].as_str() else {
        return default_pair;
    };
    let Some(start_color) = ColorG::parse_any_color(start) else {
        return default_pair;
    };
    let Some(stop_color) = ColorG::parse_any_color(stop) else {
        return default_pair;
    };
    (start_color, stop_color)
}
macro_rules! update_style {
    ($self_style:expr, $yaml:ident, $key:expr) => {
        if let Some(color) = read_yaml_string($yaml, $key) {
            $self_style = crate::config::str_to_ratatui(color).into();
        }
    };
}

fn read_yaml_string(yaml: &Value, key: &str) -> Option<String> {
    yaml[key].as_str().map(|s| s.to_string())
}

/// Holds configurable colors for every kind of file.
/// "Normal" files are displayed with a different color by extension.
#[derive(Debug, Clone)]
pub struct FileStyle {
    /// Color for `directory` files.
    pub directory: Style,
    /// Style for `block` files.
    pub block: Style,
    /// Style for `char` files.
    pub char: Style,
    /// Style for `fifo` files.
    pub fifo: Style,
    /// Style for `socket` files.
    pub socket: Style,
    /// Style for `symlink` files.
    pub symlink: Style,
    /// Style for broken `symlink` files.
    pub broken: Style,
}

impl FileStyle {
    fn new() -> Self {
        Self {
            directory: Color::Red.into(),
            block: Color::Yellow.into(),
            char: Color::Green.into(),
            fifo: Color::Blue.into(),
            socket: Color::Cyan.into(),
            symlink: Color::Magenta.into(),
            broken: Color::White.into(),
        }
    }

    /// Update every color from a yaml value (read from the config file).
    fn update_values(&mut self, yaml: &Value) {
        update_style!(self.directory, yaml, "directory");
        update_style!(self.block, yaml, "block");
        update_style!(self.char, yaml, "char");
        update_style!(self.fifo, yaml, "fifo");
        update_style!(self.socket, yaml, "socket");
        update_style!(self.symlink, yaml, "symlink");
        update_style!(self.broken, yaml, "broken");
    }

    fn update_from_config(&mut self) {
        let Ok(file) = File::open(std::path::Path::new(&tilde(CONFIG_PATH).to_string())) else {
            return;
        };
        let Ok(yaml) = from_reader::<File, Value>(file) else {
            return;
        };
        self.update_values(&yaml["colors"]);
    }

    pub fn from_config() -> Self {
        let mut style = Self::default();
        style.update_from_config();
        style
    }
}

impl Default for FileStyle {
    fn default() -> Self {
        Self::new()
    }
}

/// Different styles for decorating the menus.
pub struct MenuStyle {
    pub first: Style,
    pub second: Style,
    pub selected_border: Style,
    pub inert_border: Style,
    pub palette_1: Style,
    pub palette_2: Style,
    pub palette_3: Style,
    pub palette_4: Style,
}

impl Default for MenuStyle {
    fn default() -> Self {
        Self {
            first: Color::Rgb(45, 250, 209).into(),
            second: Color::Rgb(230, 189, 87).into(),
            selected_border: Color::Rgb(45, 250, 209).into(),
            inert_border: Color::Rgb(248, 248, 248).into(),
            palette_1: Color::Rgb(45, 250, 209).into(),
            palette_2: Color::Rgb(230, 189, 87).into(),
            palette_3: Color::Rgb(230, 167, 255).into(),
            palette_4: Color::Rgb(59, 204, 255).into(),
        }
    }
}

impl MenuStyle {
    pub fn update(mut self) -> Self {
        if let Ok(file) = File::open(path::Path::new(&tilde(CONFIG_PATH).to_string())) {
            if let Ok(yaml) = from_reader::<File, Value>(file) {
                let menu_colors = &yaml["colors"];
                update_style!(self.first, menu_colors, "header_first");
                update_style!(self.second, menu_colors, "header_second");
                update_style!(self.selected_border, menu_colors, "selected_border");
                update_style!(self.inert_border, menu_colors, "inert_border");
                update_style!(self.palette_1, menu_colors, "palette_1");
                update_style!(self.palette_2, menu_colors, "palette_2");
                update_style!(self.palette_3, menu_colors, "palette_3");
                update_style!(self.palette_4, menu_colors, "palette_4");
            }
        }
        self
    }

    #[inline]
    pub const fn palette(&self) -> [Style; 4] {
        [
            self.palette_1,
            self.palette_2,
            self.palette_3,
            self.palette_4,
        ]
    }

    #[inline]
    pub const fn palette_size(&self) -> usize {
        self.palette().len()
    }
}

/// Name of the syntect theme used.
#[derive(Debug)]
pub struct SyntectTheme {
    pub name: String,
}

impl Default for SyntectTheme {
    fn default() -> Self {
        Self {
            name: SYNTECT_DEFAULT_THEME.to_owned(),
        }
    }
}

impl SyntectTheme {
    pub fn from_config(path: &str) -> Result<Self> {
        let Ok(file) = File::open(path::Path::new(&tilde(path).to_string())) else {
            crate::log_info!("Couldn't read config file at {path}");
            return Ok(Self::default());
        };
        let Ok(yaml) = from_reader::<File, Value>(file) else {
            return Ok(Self::default());
        };
        let Some(name) = yaml["syntect_theme"].as_str() else {
            return Ok(Self::default());
        };
        crate::log_info!("Config: found syntect theme: {name}");

        Ok(Self {
            name: name.to_string(),
        })
    }
}
