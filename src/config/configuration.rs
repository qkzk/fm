use std::{fs::File, path};

use anyhow::Result;
use serde_yml::{from_reader, Value};
use tuikit::attr::Attr;
use tuikit::attr::Color;

use crate::common::tilde;
use crate::common::{is_program_in_path, DEFAULT_TERMINAL_FLAG};
use crate::common::{CONFIG_PATH, DEFAULT_TERMINAL_APPLICATION};
use crate::config::Bindings;
use crate::config::ColorG;
use crate::io::color_to_attr;

/// Holds every configurable aspect of the application.
/// All attributes are hardcoded then updated from optional values
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
        if !terminal_currently_used.is_empty() && is_program_in_path(&terminal_currently_used) {
            self.terminal = terminal_currently_used
        } else if let Some(configured_terminal) = yaml.as_str() {
            self.terminal = configured_terminal.to_owned()
        }
    }

    fn update_terminal_flag(&mut self, terminal_flag: &Value) {
        let terminal = self.terminal();
        if let Some(terminal_flag) = read_yaml_value(terminal_flag, terminal) {
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
    let file = File::open(path::Path::new(&tilde(path).to_string()))?;
    let Ok(yaml) = from_reader(file) else {
        return Ok(config);
    };
    let _ = config.update_from_config(&yaml);
    Ok(config)
}

/// Reads the config file and parse the "palette" values.
/// The palette format looks like this (with different accepted format)
/// ```yaml
/// palette:
///   start: yellow, #ffff00, rgb(255, 255, 0)
///   stop:  #ff00ff
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
    let Some(start) = yaml["palette"]["start"].as_str() else {
        return default_pair;
    };
    let Some(stop) = yaml["palette"]["stop"].as_str() else {
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
macro_rules! update_attr {
    ($self_attr:expr, $yaml:ident, $key:expr) => {
        if let Some(color) = read_yaml_value($yaml, $key) {
            $self_attr = color_to_attr(crate::config::str_to_tuikit(color));
        }
    };
}

fn read_yaml_value(yaml: &Value, key: &str) -> Option<String> {
    yaml[key].as_str().map(|s| s.to_string())
}

/// Holds configurable colors for every kind of file.
/// "Normal" files are displayed with a different color by extension.
#[derive(Debug, Clone)]
pub struct FileAttr {
    /// Color for `directory` files.
    pub directory: Attr,
    /// Attr for `block` files.
    pub block: Attr,
    /// Attr for `char` files.
    pub char: Attr,
    /// Attr for `fifo` files.
    pub fifo: Attr,
    /// Attr for `socket` files.
    pub socket: Attr,
    /// Attr for `symlink` files.
    pub symlink: Attr,
    /// Attr for broken `symlink` files.
    pub broken: Attr,
}

impl FileAttr {
    fn new() -> Self {
        Self {
            directory: color_to_attr(Color::RED),
            block: color_to_attr(Color::YELLOW),
            char: color_to_attr(Color::GREEN),
            fifo: color_to_attr(Color::BLUE),
            socket: color_to_attr(Color::CYAN),
            symlink: color_to_attr(Color::MAGENTA),
            broken: color_to_attr(Color::WHITE),
        }
    }

    /// Update every color from a yaml value (read from the config file).
    fn update_values(&mut self, yaml: &Value) {
        update_attr!(self.directory, yaml, "directory");
        update_attr!(self.block, yaml, "block");
        update_attr!(self.char, yaml, "char");
        update_attr!(self.fifo, yaml, "fifo");
        update_attr!(self.socket, yaml, "socket");
        update_attr!(self.symlink, yaml, "symlink");
        update_attr!(self.broken, yaml, "broken");
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
        let mut attrs = Self::default();
        attrs.update_from_config();
        attrs
    }
}

impl Default for FileAttr {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MenuAttrs {
    pub first: Attr,
    pub second: Attr,
    pub selected_border: Attr,
    pub inert_border: Attr,
    pub palette_1: Attr,
    pub palette_2: Attr,
    pub palette_3: Attr,
    pub palette_4: Attr,
}

impl Default for MenuAttrs {
    fn default() -> Self {
        Self {
            first: color_to_attr(Color::Rgb(45, 250, 209)),
            second: color_to_attr(Color::Rgb(230, 189, 87)),
            selected_border: color_to_attr(Color::Rgb(45, 250, 209)),
            inert_border: color_to_attr(Color::Rgb(248, 248, 248)),
            palette_1: color_to_attr(Color::Rgb(45, 250, 209)),
            palette_2: color_to_attr(Color::Rgb(230, 189, 87)),
            palette_3: color_to_attr(Color::Rgb(230, 167, 255)),
            palette_4: color_to_attr(Color::Rgb(59, 204, 255)),
        }
    }
}

impl MenuAttrs {
    pub fn update(mut self) -> Self {
        if let Ok(file) = File::open(path::Path::new(&tilde(CONFIG_PATH).to_string())) {
            if let Ok(yaml) = from_reader::<File, Value>(file) {
                let menu_colors = &yaml["menu_colors"];
                update_attr!(self.first, menu_colors, "first");
                update_attr!(self.second, menu_colors, "second");
                update_attr!(self.selected_border, menu_colors, "selected_border");
                update_attr!(self.inert_border, menu_colors, "inert_border");
                update_attr!(self.palette_1, menu_colors, "palette_1");
                update_attr!(self.palette_2, menu_colors, "palette_2");
                update_attr!(self.palette_3, menu_colors, "palette_3");
                update_attr!(self.palette_4, menu_colors, "palette_4");
            }
        }
        self
    }

    #[inline]
    pub const fn palette(&self) -> [Attr; 4] {
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
