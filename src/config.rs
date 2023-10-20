use std::{fs::File, path};

use anyhow::Result;
use serde_yaml;
use tuikit::attr::Color;

use crate::color_cache::ColorCache;
use crate::constant_strings_paths::DEFAULT_TERMINAL_APPLICATION;
use crate::keybindings::Bindings;
use crate::utils::is_program_in_path;

/// Starting settings.
/// those values are updated from the yaml config file
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub dual: bool,
    pub full: bool,
    pub all: bool,
}

impl Settings {
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        match yaml["dual"] {
            serde_yaml::Value::Bool(false) => self.dual = false,
            _ => self.dual = true,
        }
        match yaml["full"] {
            serde_yaml::Value::Bool(false) => self.full = false,
            _ => self.full = true,
        }
        match yaml["all"] {
            serde_yaml::Value::Bool(false) => self.all = false,
            _ => self.all = true,
        }
    }
}

macro_rules! update_attribute {
    ($self_attr:expr, $yaml:ident, $key:expr) => {
        if let Some(attr) = read_yaml_value($yaml, $key) {
            $self_attr = attr;
        }
    };
}
/// Holds every configurable aspect of the application.
/// All attributes are hardcoded then updated from optional values
/// of the config file.
/// The config file is a YAML file in `~/.config/fm/config.yaml`
#[derive(Debug, Clone)]
pub struct Config {
    /// Color of every kind of file
    pub colors: Colors,
    /// The name of the terminal application. It should be installed properly.
    pub terminal: String,
    /// Configurable keybindings.
    pub binds: Bindings,
    /// Basic starting settings
    pub settings: Settings,
}

impl Config {
    /// Returns a default config with hardcoded values.
    fn new() -> Result<Self> {
        Ok(Self {
            colors: Colors::default(),
            terminal: DEFAULT_TERMINAL_APPLICATION.to_owned(),
            binds: Bindings::default(),
            settings: Settings::default(),
        })
    }
    /// Updates the config from  a configuration content.
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) -> Result<()> {
        self.colors.update_from_config(&yaml["colors"]);
        self.binds.update_normal(&yaml["keys"]);
        self.binds.update_custom(&yaml["custom"]);
        self.update_terminal(&yaml["terminal"]);
        self.settings.update_from_config(&yaml["settings"]);
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

    /// The terminal name
    pub fn terminal(&self) -> &str {
        &self.terminal
    }
}

fn read_yaml_value(yaml: &serde_yaml::value::Value, key: &str) -> Option<String> {
    yaml[key].as_str().map(|s| s.to_string())
}

/// Holds configurable colors for every kind of file.
/// "Normal" files are displayed with a different color by extension.
#[derive(Debug, Clone)]
pub struct Colors {
    /// Color for `directory` files.
    pub directory: String,
    /// Color for `block` files.
    pub block: String,
    /// Color for `char` files.
    pub char: String,
    /// Color for `fifo` files.
    pub fifo: String,
    /// Color for `socket` files.
    pub socket: String,
    /// Color for `symlink` files.
    pub symlink: String,
    /// Color for broken `symlink` files.
    pub broken: String,
    /// Colors for normal files, depending of extension
    pub color_cache: ColorCache,
}

impl Colors {
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        update_attribute!(self.directory, yaml, "directory");
        update_attribute!(self.block, yaml, "block");
        update_attribute!(self.char, yaml, "char");
        update_attribute!(self.fifo, yaml, "fifo");
        update_attribute!(self.socket, yaml, "socket");
        update_attribute!(self.symlink, yaml, "symlink");
        update_attribute!(self.broken, yaml, "broken");
    }

    fn new() -> Self {
        Self {
            directory: "red".to_owned(),
            block: "yellow".to_owned(),
            char: "green".to_owned(),
            fifo: "blue".to_owned(),
            socket: "cyan".to_owned(),
            symlink: "magenta".to_owned(),
            broken: "white".to_owned(),
            color_cache: ColorCache::default(),
        }
    }
}

impl Default for Colors {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a string color into a `tuikit::Color` instance.
pub fn str_to_tuikit(color: &str) -> Color {
    match color {
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
        _ => Color::default(),
    }
}

/// Returns a config with values from :
///
/// 1. hardcoded values
///
/// 2. configured values from `~/.config/fm/config_file_name.yaml` if those files exists.
/// If the config fle is poorly formated its simply ignored.
pub fn load_config(path: &str) -> Result<Config> {
    let mut config = Config::new()?;
    let file = File::open(path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let Ok(yaml) = serde_yaml::from_reader(file) else {
        return Ok(config);
    };
    let _ = config.update_from_config(&yaml);
    Ok(config)
}
