use std::fs::File;

use serde_yaml;
use tuikit::attr::Color;

#[derive(Debug, Clone)]
pub struct Colors {
    pub file: String,
    pub directory: String,
    pub block: String,
    pub char: String,
    pub fifo: String,
    pub socket: String,
}

impl Colors {
    pub fn from_config(yaml: &serde_yaml::value::Value) -> Self {
        let file = yaml["file"]
            .as_str()
            .map(|s| s.to_string())
            .expect("Couldn't parse config file");
        let directory = yaml["directory"]
            .as_str()
            .map(|s| s.to_string())
            .expect("Couldn't parse config file");
        let block = yaml["block"]
            .as_str()
            .map(|s| s.to_string())
            .expect("Couldn't parse config file");
        let char = yaml["char"]
            .as_str()
            .map(|s| s.to_string())
            .expect("Couldn't parse config file");
        let fifo = yaml["fifo"]
            .as_str()
            .map(|s| s.to_string())
            .expect("Couldn't parse config file");
        let socket = yaml["socket"]
            .as_str()
            .map(|s| s.to_string())
            .expect("Couldn't parse config file");
        Self {
            file,
            directory,
            block,
            char,
            fifo,
            socket,
        }
    }
}

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

pub fn load_file(file: &str) -> serde_yaml::Value {
    let file = File::open(file).expect("Unable to open file");
    serde_yaml::from_reader(file).expect("Couldn't read yaml file")
}
