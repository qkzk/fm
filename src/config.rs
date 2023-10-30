use std::{fs::File, path};

use anyhow::Result;
use serde_yaml;
// use tuikit::attr::Color;

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
    pub preview: bool,
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
        match yaml["preview"] {
            serde_yaml::Value::Bool(true) => self.all = true,
            _ => self.all = false,
        }
    }
}

/// Holds every configurable aspect of the application.
/// All attributes are hardcoded then updated from optional values
/// of the config file.
/// The config file is a YAML file in `~/.config/fm/config.yaml`
#[derive(Debug, Clone)]
pub struct Config {
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
            terminal: DEFAULT_TERMINAL_APPLICATION.to_owned(),
            binds: Bindings::default(),
            settings: Settings::default(),
        })
    }
    /// Updates the config from  a configuration content.
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) -> Result<()> {
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
