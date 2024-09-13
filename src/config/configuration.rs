use std::{fs::File, path};

use anyhow::Result;
use serde_yml::{from_reader, Value};
use tuikit::attr::Attr;
use tuikit::attr::Color;

use crate::common::tilde;
use crate::common::{is_program_in_path, DEFAULT_TERMINAL_FLAG};
use crate::common::{CONFIG_PATH, DEFAULT_TERMINAL_APPLICATION};
use crate::config::Bindings;
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
/// rgb and hexadecimal formats should never fail.
/// Other formats are unknown.
/// rgb( 123,   78,          0) -> Color::Rgb(123, 78, 0)
/// #FF00FF -> Color::Rgb(255, 0, 255)
/// Unreadable colors are replaced by `Color::default()` which is white.
fn parse_rgb_color(color: &str) -> Color {
    if let Some(triplet) = parse_text_triplet(color) {
        return Color::Rgb(triplet.0, triplet.1, triplet.2);
    }
    Color::default()
}

fn parse_text_triplet(color: &str) -> Option<(u8, u8, u8)> {
    let color = color.to_lowercase();
    if color.starts_with("rgb(") && color.ends_with(')') {
        return parse_rgb_triplet(&color);
    } else if color.starts_with('#') && color.len() >= 7 {
        return parse_hex_triplet(&color);
    }
    None
}

fn parse_rgb_triplet(color: &str) -> Option<(u8, u8, u8)> {
    let triplet: Vec<u8> = color
        .replace("rgb(", "")
        .replace([')', ' '], "")
        .trim()
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    if triplet.len() == 3 {
        return Some((triplet[0], triplet[1], triplet[2]));
    }
    None
}

fn parse_hex_triplet(color: &str) -> Option<(u8, u8, u8)> {
    let r = parse_hex_byte(&color[1..3])?;
    let g = parse_hex_byte(&color[3..5])?;
    let b = parse_hex_byte(&color[5..7])?;
    Some((r, g, b))
}

fn parse_hex_byte(byte: &str) -> Option<u8> {
    u8::from_str_radix(byte, 16).ok()
}

macro_rules! update_attribute {
    ($self_attr:expr, $yaml:ident, $key:expr) => {
        if let Some(attr) = read_yaml_value($yaml, $key) {
            $self_attr = str_to_tuikit(attr);
        }
    };
}

fn read_yaml_value(yaml: &Value, key: &str) -> Option<String> {
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
    pub fn update_from_config(&mut self, yaml: &Value) {
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

pub fn load_color_from_config(key: &str) -> Option<(u8, u8, u8)> {
    let config_path = &tilde(CONFIG_PATH).to_string();
    let config_path = std::path::Path::new(config_path);

    if let Ok(file) = File::open(config_path) {
        if let Ok(yaml) = from_reader::<File, Value>(file) {
            let palette = yaml.get("palette")?;
            if let Some(color) = palette.get(key)?.as_str() {
                return parse_text_triplet(color);
            }
        }
    }
    None
}

pub struct MenuColors {
    pub first: Color,
    pub second: Color,
    pub selected_border: Color,
    pub inert_border: Color,
    pub palette_1: Color,
    pub palette_2: Color,
    pub palette_3: Color,
    pub palette_4: Color,
}

impl Default for MenuColors {
    fn default() -> Self {
        Self {
            first: Color::Rgb(45, 250, 209),
            second: Color::Rgb(230, 189, 87),
            selected_border: Color::Rgb(45, 250, 209),
            inert_border: Color::Rgb(248, 248, 248),
            palette_1: Color::Rgb(45, 250, 209),
            palette_2: Color::Rgb(230, 189, 87),
            palette_3: Color::Rgb(230, 167, 255),
            palette_4: Color::Rgb(59, 204, 255),
        }
    }
}

impl MenuColors {
    pub fn update(mut self) -> Self {
        if let Ok(file) = File::open(path::Path::new(&tilde(CONFIG_PATH).to_string())) {
            if let Ok(yaml) = from_reader::<File, Value>(file) {
                let menu_colors = &yaml["menu_colors"];
                update_attribute!(self.first, menu_colors, "first");
                update_attribute!(self.second, menu_colors, "second");
                update_attribute!(self.selected_border, menu_colors, "selected_border");
                update_attribute!(self.inert_border, menu_colors, "inert_border");
                update_attribute!(self.palette_1, menu_colors, "palette_1");
                update_attribute!(self.palette_2, menu_colors, "palette_2");
                update_attribute!(self.palette_3, menu_colors, "palette_3");
                update_attribute!(self.palette_4, menu_colors, "palette_4");
            }
        }
        self
    }

    #[inline]
    pub const fn palette(&self) -> [Attr; 4] {
        [
            color_to_attr(self.palette_1),
            color_to_attr(self.palette_2),
            color_to_attr(self.palette_3),
            color_to_attr(self.palette_4),
        ]
    }

    #[inline]
    pub const fn palette_size(&self) -> usize {
        self.palette().len()
    }
}
