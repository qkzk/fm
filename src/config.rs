use std::{fs::File, path};

use serde_yaml;
use tuikit::attr::Color;

use crate::keybindings::Keybindings;

/// Holds every configurable aspect of the application.
/// All attributes are hardcoded then updated from optional values
/// of the config file.
/// The config file is a YAML file in `~/.config/fm/config.yaml`
/// Default config file looks like this and is easier to read in code directly.
/// # the terminal must be installed
/// terminal: st
/// colors:
///   # white, black, red, green, blue, yellow, cyan, magenta
///   # light_white, light_black, light_red, light_green, light_blue, light_yellow, light_cyan, light_magenta
///   file: white
///   directory: red
///   block: yellow
///   char: green
///   fifo: blue
///   socket: cyan
///   symlink: magenta
/// keybindings:
///   # only ASCII char keys are allowed
///   # ASCII letters must be lowercase
///   # non ASCII letters char must be in single quotes like this '*'
///   toggle_hidden: a
///   copy_paste: c
///   cut_paste: p
///   delete: x
///   symlink: S
///   chmod: m
///   exec: e
///   newdir: d
///   newfile: n
///   rename: r
///   clear_flags: u
///   toggle_flag: ' '
///   shell: s
///   open_file: o
///   help: h
///   search: '/'
///   quit: q
///   goto: g
///   flag_all: '*'
///   reverse_flags: v
///   regex_match: w
///   jump: j
///   nvim: i
///   sort_by: O
///   preview: P
///   shortcut: S
#[derive(Debug, Clone)]
pub struct Config {
    /// Color of every kind of file
    pub colors: Colors,
    /// Configurable keybindings.
    pub keybindings: Keybindings,
    /// Terminal used to open file
    pub terminal: String,
}

impl Config {
    /// Returns a default config with hardcoded values.
    fn new() -> Self {
        Self {
            colors: Colors::default(),
            keybindings: Keybindings::default(),
            terminal: "st".to_owned(),
        }
    }

    /// Updates the config from  a configuration content.
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        self.colors.update_from_config(&yaml["colors"]);
        self.keybindings.update_from_config(&yaml["keybindings"]);
        if let Some(terminal) = yaml["terminal"].as_str().map(|s| s.to_string()) {
            self.terminal = terminal;
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Colors {
    pub file: String,
    pub directory: String,
    pub block: String,
    pub char: String,
    pub fifo: String,
    pub socket: String,
    pub symlink: String,
}

impl Colors {
    /// Update a config from a YAML content.
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        if let Some(file) = yaml["file"].as_str().map(|s| s.to_string()) {
            self.file = file;
        }
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

    /// Every default color is hardcoded.
    fn new() -> Self {
        Self {
            file: "white".to_owned(),
            directory: "red".to_owned(),
            block: "yellow".to_owned(),
            char: "green".to_owned(),
            fifo: "blue".to_owned(),
            socket: "cyan".to_owned(),
            symlink: "magenta".to_owned(),
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
/// 2. configured values from `~/.config/fm/config.yaml` if the file exists.
pub fn load_config(path: &str) -> Config {
    let mut config = Config::default();

    if let Ok(file) = File::open(path::Path::new(&shellexpand::tilde(path).to_string())) {
        if let Ok(yaml) = serde_yaml::from_reader(file) {
            config.update_from_config(&yaml);
        }
    }

    config
}
