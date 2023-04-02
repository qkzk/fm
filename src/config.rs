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
    }
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
        self.binds.update_from_config(&yaml["keys"]);
        self.binds.update_custom(&yaml["custom"]);
        self.terminal = Self::set_terminal(&yaml["terminal"])?;
        self.settings.update_from_config(&yaml["settings"]);
        Ok(())
    }

    /// First we try to use the current terminal. If it's a fake one (ie. inside neovim float term),
    /// we look for the configured one,
    /// else we use the hardcoded one.
    fn set_terminal(yaml: &serde_yaml::value::Value) -> Result<String> {
        let terminal_currently_used = std::env::var("TERM").unwrap_or_default();
        if !terminal_currently_used.is_empty() && is_program_in_path(&terminal_currently_used) {
            Ok(terminal_currently_used)
        } else if let Some(configured_terminal) = yaml.as_str() {
            Ok(configured_terminal.to_owned())
        } else {
            Ok(DEFAULT_TERMINAL_APPLICATION.to_owned())
        }
    }

    /// The terminal name
    pub fn terminal(&self) -> &str {
        &self.terminal
    }
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
    /// Colors for normal files, depending of extension
    pub color_cache: ColorCache,
}

impl Colors {
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        if let Some(directory) = yaml["directory"].as_str().map(|s| s.to_string()) {
            self.directory = directory;
        }
        if let Some(block) = yaml["block"].as_str().map(|s| s.to_string()) {
            self.block = block;
        }
        if let Some(char) = yaml["char"].as_str().map(|s| s.to_string()) {
            self.char = char;
        }
        if let Some(fifo) = yaml["fifo"].as_str().map(|s| s.to_string()) {
            self.fifo = fifo;
        }
        if let Some(socket) = yaml["socket"].as_str().map(|s| s.to_string()) {
            self.socket = socket;
        }
        if let Some(symlink) = yaml["symlink"].as_str().map(|s| s.to_string()) {
            self.symlink = symlink;
        }
    }

    fn new() -> Self {
        Self {
            directory: "red".to_owned(),
            block: "yellow".to_owned(),
            char: "green".to_owned(),
            fifo: "blue".to_owned(),
            socket: "cyan".to_owned(),
            symlink: "magenta".to_owned(),
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
pub fn load_config(path: &str) -> Result<Config> {
    let mut config = Config::new()?;

    if let Ok(file) = File::open(path::Path::new(&shellexpand::tilde(path).to_string())) {
        if let Ok(yaml) = serde_yaml::from_reader(file) {
            config.update_from_config(&yaml)?;
        }
    }

    Ok(config)
}
