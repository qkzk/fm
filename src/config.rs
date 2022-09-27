extern crate shellexpand;

use std::{fs::File, path};

use serde_yaml;
use tuikit::attr::Color;

static HOME_CONFIG_DIR: &str = "~/.config/fm";
static ETC_CONFIG_DIR: &str = "/etc/fm";
// pub static CONFIG_FILE: &str = "/home/quentin/gclem/dev/rust/fm/config.yaml";
pub static CONFIG_NAME: &str = "config.yaml";

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
        let symlink = yaml["symlink"]
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
            symlink,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Keybindings {
    pub toggle_hidden: char,
    pub copy_paste: char,
    pub cut_paste: char,
    pub delete: char,
    pub chmod: char,
    pub exec: char,
    pub newdir: char,
    pub newfile: char,
    pub rename: char,
    pub clear_flags: char,
    pub toggle_flag: char,
    pub shell: char,
    pub open_file: char,
    pub help: char,
    pub search: char,
    pub quit: char,
    pub goto: char,
    pub flag_all: char,
    pub reverse_flags: char,
    pub regex_match: char,
    pub jump: char,
}

impl Keybindings {
    pub fn new(yaml: &serde_yaml::value::Value) -> Self {
        let toggle_hidden = yaml["toggle_hidden"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "a".to_string())
            .chars()
            .next()
            .unwrap_or('a');
        let copy_paste = yaml["copy_paste"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "c".to_string())
            .chars()
            .next()
            .unwrap_or('c');
        let cut_paste = yaml["cut_paste"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "p".to_string())
            .chars()
            .next()
            .unwrap_or('p');
        let delete = yaml["delete"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "x".to_string())
            .chars()
            .next()
            .unwrap_or('x');
        let chmod = yaml["chmod"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "m".to_string())
            .chars()
            .next()
            .unwrap_or('m');
        let exec = yaml["exec"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "e".to_string())
            .chars()
            .next()
            .unwrap_or('e');
        let newdir = yaml["newdir"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "d".to_string())
            .chars()
            .next()
            .unwrap_or('d');
        let newfile = yaml["newfile"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "n".to_string())
            .chars()
            .next()
            .unwrap_or('n');
        let rename = yaml["rename"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "r".to_string())
            .chars()
            .next()
            .unwrap_or('r');
        let clear_flags = yaml["clear_flags"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "u".to_string())
            .chars()
            .next()
            .unwrap_or('u');
        let toggle_flag = yaml["toggle_flag"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| " ".to_string())
            .chars()
            .next()
            .unwrap_or(' ');
        let shell = yaml["shell"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "s".to_string())
            .chars()
            .next()
            .unwrap_or('s');
        let open_file = yaml["open_file"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "o".to_string())
            .chars()
            .next()
            .unwrap_or('o');
        let help = yaml["help"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "h".to_string())
            .chars()
            .next()
            .unwrap_or('h');
        let search = yaml["search"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "/".to_string())
            .chars()
            .next()
            .unwrap_or('/');
        let quit = yaml["quit"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "q".to_string())
            .chars()
            .next()
            .unwrap_or('q');
        let goto = yaml["goto"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "g".to_string())
            .chars()
            .next()
            .unwrap_or('g');
        let flag_all = yaml["flag_all"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "*".to_string())
            .chars()
            .next()
            .unwrap_or('*');
        let reverse_flags = yaml["reverse_flags"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "v".to_string())
            .chars()
            .next()
            .unwrap_or('v');
        let regex_match = yaml["regex_match"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "w".to_string())
            .chars()
            .next()
            .unwrap_or('w');
        let jump = yaml["jump"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "j".to_string())
            .chars()
            .next()
            .unwrap_or('j');
        Self {
            toggle_hidden,
            copy_paste,
            cut_paste,
            delete,
            chmod,
            exec,
            newdir,
            newfile,
            rename,
            clear_flags,
            toggle_flag,
            shell,
            open_file,
            help,
            search,
            quit,
            goto,
            flag_all,
            reverse_flags,
            regex_match,
            jump,
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
    let file = File::open(file).unwrap();
    serde_yaml::from_reader(file).unwrap()
}

fn get_current_working_dir() -> std::io::Result<std::path::PathBuf> {
    std::env::current_dir()
}

pub fn search_config() -> serde_yaml::Value {
    let user_string = shellexpand::tilde(&HOME_CONFIG_DIR).to_string();
    let mut user_path = path::PathBuf::new();
    user_path.push(user_string);

    let etc_config_string = shellexpand::tilde(&ETC_CONFIG_DIR).to_string();
    let mut etc_config_path = path::PathBuf::new();
    etc_config_path.push(etc_config_string);

    let dev_config = std::env::var("CONFIG_FILE").unwrap_or("".into());
    let mut dev_path = path::PathBuf::new();
    dev_path.push(dev_config);

    // let cur_path = get_current_working_dir().unwrap();
    let cur_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    eprintln!("current exe in {:?}", cur_path);

    for mut path in [dev_path] {
        // for mut path in [user_path, etc_config_path, dev_path, cur_path] {
        path.push(CONFIG_NAME);
        eprintln!("trying {:?}", &path);
        if let Ok(file) = File::open(path) {
            if let Ok(config) = serde_yaml::from_reader(file) {
                return config;
            }
        }
    }
    std::process::exit(2);
}
