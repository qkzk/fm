use std::{fs::File, path};

use anyhow::Result;
use clap::Parser;
use serde_yaml;
use tuikit::attr::Color;

use crate::common::{is_program_in_path, DEFAULT_TERMINAL_FLAG};
use crate::common::{CONFIG_PATH, DEFAULT_TERMINAL_APPLICATION};
use crate::config::Bindings;
use crate::config::Colorer;

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
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) -> Result<()> {
        self.binds.update_normal(&yaml["keys"]);
        self.binds.update_custom(&yaml["custom"]);
        self.update_terminal(&yaml["terminal"]);
        self.update_terminal_flag(&yaml["terminal_emulator_flags"]);
        Ok(())
    }

    /// First we try to use the current terminal. If it's a fake one (ie. inside neovim float term),
    /// we look for the configured one,
    /// else nothing is done.
    fn update_terminal(&mut self, yaml: &serde_yaml::value::Value) {
        let terminal_currently_used = std::env::var("TERM").unwrap_or_default();
        if !terminal_currently_used.is_empty() && is_program_in_path(&terminal_currently_used) {
            self.terminal = terminal_currently_used
        } else if let Some(configured_terminal) = yaml.as_str() {
            self.terminal = configured_terminal.to_owned()
        }
    }

    fn update_terminal_flag(&mut self, terminal_flag: &serde_yaml::value::Value) {
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
/// If the config fle is poorly formated its simply ignored.
pub fn load_config(path: &str) -> Result<Config> {
    let mut config = Config::default();
    let file = File::open(path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let Ok(yaml) = serde_yaml::from_reader(file) else {
        return Ok(config);
    };
    let _ = config.update_from_config(&yaml);
    Ok(config)
}

/// Convert a string color into a `tuikit::Color` instance.
pub fn str_to_tuikit<S>(color: S) -> Color
where
    S: AsRef<str>,
{
    match color.as_ref() {
        "white" => Color::WHITE,
        "red" => Color::RED,
        "green" => Color::GREEN,
        "blue" => Color::BLUE,
        "yellow" => Color::YELLOW,
        "cyan" => Color::CYAN,
        "magenta" => Color::MAGENTA,
        "black" => Color::BLACK,
        "light_white" => Color::LIGHT_WHITE,
        "light_red" => Color::LIGHT_RED,
        "light_green" => Color::LIGHT_GREEN,
        "light_blue" => Color::LIGHT_BLUE,
        "light_yellow" => Color::LIGHT_YELLOW,
        "light_cyan" => Color::LIGHT_CYAN,
        "light_magenta" => Color::LIGHT_MAGENTA,
        "light_black" => Color::LIGHT_BLACK,
        color => parse_rgb_color(color),
    }
}

/// Tries to parse an unknown color into a `Color::Rgb(u8, u8, u8)`
/// rgb format should never fail.
/// Other formats are unknown.
/// rgb( 123,   78,          0) -> Color::Rgb(123, 78, 0)
/// #FF00FF -> Color::default()
/// Unreadable colors are replaced by `Color::default()` which is white.
fn parse_rgb_color(color: &str) -> Color {
    let color = color.to_lowercase();
    if color.starts_with("rgb(") && color.ends_with(')') {
        let triplet: Vec<u8> = color
            .replace("rgb(", "")
            .replace([')', ' '], "")
            .trim()
            .split(',')
            .filter_map(|s| s.parse().ok())
            .collect();
        if triplet.len() == 3 {
            return Color::Rgb(triplet[0], triplet[1], triplet[2]);
        }
    }

    Color::default()
}

macro_rules! update_attribute {
    ($self_attr:expr, $yaml:ident, $key:expr) => {
        if let Some(attr) = read_yaml_value($yaml, $key) {
            $self_attr = str_to_tuikit(attr);
        }
    };
}
fn read_yaml_value(yaml: &serde_yaml::value::Value, key: &str) -> Option<String> {
    yaml[key].as_str().map(|s| s.to_string())
}

/// Holds configurable colors for every kind of file.
/// "Normal" files are displayed with a different color by extension.
#[derive(Debug, Clone)]
pub struct Colors {
    /// Color for `directory` files.
    pub directory: Color,
    /// Color for `block` files.
    pub block: Color,
    /// Color for `char` files.
    pub char: Color,
    /// Color for `fifo` files.
    pub fifo: Color,
    /// Color for `socket` files.
    pub socket: Color,
    /// Color for `symlink` files.
    pub symlink: Color,
    /// Color for broken `symlink` files.
    pub broken: Color,
}

impl Colors {
    fn new() -> Self {
        Self {
            directory: Color::RED,
            block: Color::YELLOW,
            char: Color::GREEN,
            fifo: Color::BLUE,
            socket: Color::CYAN,
            symlink: Color::MAGENTA,
            broken: Color::WHITE,
        }
    }

    /// Update every color from a yaml value (read from the config file).
    pub fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        update_attribute!(self.directory, yaml, "directory");
        update_attribute!(self.block, yaml, "block");
        update_attribute!(self.char, yaml, "char");
        update_attribute!(self.fifo, yaml, "fifo");
        update_attribute!(self.socket, yaml, "socket");
        update_attribute!(self.symlink, yaml, "symlink");
        update_attribute!(self.broken, yaml, "broken");
    }
}

impl Default for Colors {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    /// Colors read from the config file.
    /// We define a colors for every kind of file except normal files.
    /// Colors for normal files are calculated from their extension and
    /// are greens or blues.
    ///
    /// Colors are setup on start and never change afterwards.
    /// Since many functions use colors for formating, using `lazy_static`
    /// avoids to pass them everytime.
    pub static ref COLORS: Colors = {
        let mut colors = Colors::default();
        if let Ok(file) = File::open(path::Path::new(&shellexpand::tilde(CONFIG_PATH).to_string())) {
            if let Ok(yaml)  = serde_yaml::from_reader::<std::fs::File, serde_yaml::value::Value>(file) {
                colors.update_from_config(&yaml["colors"]);
            };
        };
        colors
    };
}

lazy_static::lazy_static! {
    /// Defines a palette which will color the "normal" files based on their extension.
    /// We try to read a yaml value and pick one of 3 palettes :
    /// "red-green", "red-blue" and "green-blue" which is the default.
    pub static ref COLORER: fn(usize) -> Color = {
        let mut colorer = Colorer::color_green_blue as fn(usize) -> Color;
        if let Ok(file) = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(CONFIG_PATH).to_string())) {
            if let Ok(yaml)  = serde_yaml::from_reader::<std::fs::File, serde_yaml::value::Value>(file) {
                if let Some(palette) = yaml["palette"].as_str() {
                    match palette {
                        "red-blue" => {colorer = Colorer::color_red_blue as fn(usize) -> Color;},
                        "red-green" => {colorer = Colorer::color_red_green as fn(usize) -> Color;},
                        _ => ()
                    }
                }
            };
        };
        colorer
    };
}

lazy_static::lazy_static! {
    /// Starting folder of the application. Read from arguments `-P` or `.`.
    pub static ref START_FOLDER: std::path::PathBuf =
        std::fs::canonicalize(crate::io::Args::parse().path).unwrap_or_default();
}
