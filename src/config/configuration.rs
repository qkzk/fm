use std::{fs::File, path};

use anyhow::Result;
use ratatui::style::{Color, Style};
use serde_yml::{from_reader, Value};

use crate::common::{
    is_in_path, tilde, CONFIG_PATH, DEFAULT_TERMINAL_APPLICATION, DEFAULT_TERMINAL_FLAG,
};
use crate::config::{Bindings, ColorG};
use crate::io::color_to_style;

/// Holds every configurable aspect of the application.
/// All styles are hardcoded then updated from optional values
/// of the config file.
/// The config file is a YAML file in `~/.config/fm/config.yaml`
#[derive(Debug, Clone)]
pub struct Config {
    /// The name of the terminal application. It should be installed properly.
    pub terminal: String,
    /// terminal flag to run a command with the terminal emulator
    pub terminal_flag: String,
    /// Configurable keybindings.
    pub binds: Bindings,
}

impl Default for Config {
    /// Returns a default config with hardcoded values.
    fn default() -> Self {
        Self {
            terminal: DEFAULT_TERMINAL_APPLICATION.to_owned(),
            binds: Bindings::default(),
            terminal_flag: DEFAULT_TERMINAL_FLAG.to_owned(),
        }
    }
}

impl Config {
    /// Updates the config from  a configuration content.
    fn update_from_config(&mut self, yaml: &Value) -> Result<()> {
        self.binds.update_normal(&yaml["keys"]);
        self.binds.update_custom(&yaml["custom"]);
        self.update_terminal(&yaml["terminal"]);
        self.update_terminal_flag(&yaml["terminal_emulator_flags"]);
        Ok(())
    }

    /// First we try to use the current terminal. If it's a fake one (ie. inside neovim float term),
    /// we look for the configured one,
    /// else nothing is done.
    fn update_terminal(&mut self, yaml: &Value) {
        let terminal_currently_used = std::env::var("TERM").unwrap_or_default();
        if !terminal_currently_used.is_empty() && is_in_path(&terminal_currently_used) {
            self.terminal = terminal_currently_used
        } else if let Some(configured_terminal) = yaml.as_str() {
            self.terminal = configured_terminal.to_owned()
        }
    }

    fn update_terminal_flag(&mut self, terminal_flag: &Value) {
        let terminal = self.terminal();
        if let Some(terminal_flag) = read_yaml_string(terminal_flag, terminal) {
            self.terminal_flag = terminal_flag.as_str().to_owned();
            crate::log_info!(
                "updated terminal_flag for {terminal} using {tf}",
                terminal = self.terminal,
                tf = self.terminal_flag
            );
        } else {
            crate::log_info!(
                "Couldn't find {terminal} in config file. Using default",
                terminal = self.terminal
            )
        }
    }

    /// The terminal name
    pub fn terminal(&self) -> &str {
        &self.terminal
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
            $self_style = color_to_style(crate::config::str_to_ratatui(color));
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
            directory: color_to_style(Color::Red),
            block: color_to_style(Color::Yellow),
            char: color_to_style(Color::Green),
            fifo: color_to_style(Color::Blue),
            socket: color_to_style(Color::Cyan),
            symlink: color_to_style(Color::Magenta),
            broken: color_to_style(Color::White),
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
            first: color_to_style(Color::Rgb(45, 250, 209)),
            second: color_to_style(Color::Rgb(230, 189, 87)),
            selected_border: color_to_style(Color::Rgb(45, 250, 209)),
            inert_border: color_to_style(Color::Rgb(248, 248, 248)),
            palette_1: color_to_style(Color::Rgb(45, 250, 209)),
            palette_2: color_to_style(Color::Rgb(230, 189, 87)),
            palette_3: color_to_style(Color::Rgb(230, 167, 255)),
            palette_4: color_to_style(Color::Rgb(59, 204, 255)),
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
