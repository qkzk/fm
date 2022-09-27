extern crate shellexpand;

use std::{fs::File, path};

use serde_yaml;
use tuikit::attr::Color;

static HOME_CONFIG_DIR: &str = "~/.config/fm";
static ETC_CONFIG_DIR: &str = "/etc/fm";
// pub static CONFIG_FILE: &str = "/home/quentin/gclem/dev/rust/fm/config.yaml";
pub static CONFIG_NAME: &str = "config.yaml";

fn read_yaml_value(yaml: &serde_yaml::value::Value, key: String) -> Option<String> {
    yaml[&key].as_str().map(|s| s.to_string())
}

pub fn string_to_char(s: String) -> char {
    s.chars().next().unwrap()
}

#[derive(Debug)]
pub struct OColors {
    pub file: Option<String>,
    pub directory: Option<String>,
    pub block: Option<String>,
    pub char: Option<String>,
    pub fifo: Option<String>,
    pub socket: Option<String>,
    pub symlink: Option<String>,
}

impl OColors {
    fn new(yaml: &serde_yaml::value::Value) -> Self {
        Self {
            file: read_yaml_value(yaml, "file".to_owned()),
            directory: read_yaml_value(yaml, "directory".to_owned()),
            block: read_yaml_value(yaml, "block".to_owned()),
            char: read_yaml_value(yaml, "char".to_owned()),
            socket: read_yaml_value(yaml, "socket".to_owned()),
            fifo: read_yaml_value(yaml, "fifo".to_owned()),
            symlink: read_yaml_value(yaml, "symlink".to_owned()),
        }
    }

    fn is_complete(&self) -> bool {
        self.file.is_some()
            && self.directory.is_some()
            && self.block.is_some()
            && self.char.is_some()
            && self.socket.is_some()
            && self.fifo.is_some()
            && self.symlink.is_some()
    }

    fn update(&mut self, other: Self) {
        if other.file.is_some() {
            self.file = other.file
        }
        if other.directory.is_some() {
            self.directory = other.directory
        }
        if other.block.is_some() {
            self.block = other.block
        }
        if other.char.is_some() {
            self.char = other.char
        }
        if other.fifo.is_some() {
            self.fifo = other.fifo
        }
        if other.socket.is_some() {
            self.socket = other.socket
        }
        if other.symlink.is_some() {
            self.symlink = other.symlink
        }
    }
}

#[derive(Debug)]
pub struct OKeybindings {
    pub toggle_hidden: Option<String>,
    pub copy_paste: Option<String>,
    pub cut_paste: Option<String>,
    pub delete: Option<String>,
    pub chmod: Option<String>,
    pub exec: Option<String>,
    pub newdir: Option<String>,
    pub newfile: Option<String>,
    pub rename: Option<String>,
    pub clear_flags: Option<String>,
    pub toggle_flag: Option<String>,
    pub shell: Option<String>,
    pub open_file: Option<String>,
    pub help: Option<String>,
    pub search: Option<String>,
    pub quit: Option<String>,
    pub goto: Option<String>,
    pub flag_all: Option<String>,
    pub reverse_flags: Option<String>,
    pub regex_match: Option<String>,
    pub jump: Option<String>,
}

impl OKeybindings {
    fn new(yaml: &serde_yaml::value::Value) -> Self {
        Self {
            toggle_hidden: read_yaml_value(yaml, "toggle_hidden".to_owned()),
            copy_paste: read_yaml_value(yaml, "copy_paste".to_owned()),
            cut_paste: read_yaml_value(yaml, "cut_paste".to_owned()),
            delete: read_yaml_value(yaml, "delete".to_owned()),
            chmod: read_yaml_value(yaml, "chmod".to_owned()),
            exec: read_yaml_value(yaml, "exec".to_owned()),
            newdir: read_yaml_value(yaml, "newdir".to_owned()),
            newfile: read_yaml_value(yaml, "newfile".to_owned()),
            rename: read_yaml_value(yaml, "rename".to_owned()),
            clear_flags: read_yaml_value(yaml, "clear_flags".to_owned()),
            toggle_flag: read_yaml_value(yaml, "toggle_flag".to_owned()),
            shell: read_yaml_value(yaml, "shell".to_owned()),
            open_file: read_yaml_value(yaml, "open_file".to_owned()),
            help: read_yaml_value(yaml, "help".to_owned()),
            search: read_yaml_value(yaml, "search".to_owned()),
            quit: read_yaml_value(yaml, "quit".to_owned()),
            goto: read_yaml_value(yaml, "goto".to_owned()),
            flag_all: read_yaml_value(yaml, "flag_all".to_owned()),
            reverse_flags: read_yaml_value(yaml, "reverse_flags".to_owned()),
            regex_match: read_yaml_value(yaml, "regex_match".to_owned()),
            jump: read_yaml_value(yaml, "jump".to_owned()),
        }
    }

    fn is_complete(&self) -> bool {
        self.toggle_hidden.is_some()
            && self.copy_paste.is_some()
            && self.cut_paste.is_some()
            && self.delete.is_some()
            && self.chmod.is_some()
            && self.exec.is_some()
            && self.newdir.is_some()
            && self.newfile.is_some()
            && self.rename.is_some()
            && self.clear_flags.is_some()
            && self.toggle_flag.is_some()
            && self.shell.is_some()
            && self.open_file.is_some()
            && self.help.is_some()
            && self.search.is_some()
            && self.quit.is_some()
            && self.goto.is_some()
            && self.flag_all.is_some()
            && self.reverse_flags.is_some()
            && self.regex_match.is_some()
            && self.jump.is_some()
    }

    fn update(&mut self, other: Self) {
        if other.copy_paste.is_some() {
            self.copy_paste = other.copy_paste
        }
        if other.cut_paste.is_some() {
            self.cut_paste = other.cut_paste
        }
        if other.delete.is_some() {
            self.delete = other.delete
        }
        if other.chmod.is_some() {
            self.chmod = other.chmod
        }
        if other.exec.is_some() {
            self.exec = other.exec
        }
        if other.newdir.is_some() {
            self.newdir = other.newdir
        }
        if other.newfile.is_some() {
            self.newfile = other.newfile
        }
        if other.rename.is_some() {
            self.rename = other.rename
        }
        if other.clear_flags.is_some() {
            self.clear_flags = other.clear_flags
        }
        if other.toggle_flag.is_some() {
            self.toggle_flag = other.toggle_flag
        }
        if other.shell.is_some() {
            self.shell = other.shell
        }
        if other.open_file.is_some() {
            self.open_file = other.open_file
        }
        if other.help.is_some() {
            self.help = other.help
        }
        if other.search.is_some() {
            self.search = other.search
        }
        if other.quit.is_some() {
            self.quit = other.quit
        }
        if other.goto.is_some() {
            self.goto = other.goto
        }
        if other.flag_all.is_some() {
            self.flag_all = other.flag_all
        }
        if other.reverse_flags.is_some() {
            self.reverse_flags = other.reverse_flags
        }
        if other.regex_match.is_some() {
            self.regex_match = other.regex_match
        }
        if other.jump.is_some() {
            self.jump = other.jump
        }
    }
}

#[derive(Debug)]
pub struct OConfig {
    pub colors: OColors,
    pub keybindings: OKeybindings,
    pub terminal: Option<String>,
    pub opener: Option<String>,
}

impl OConfig {
    fn new(yaml: &serde_yaml::value::Value) -> Self {
        Self {
            colors: OColors::new(&yaml["colors"]),
            keybindings: OKeybindings::new(&yaml["keybindings"]),
            terminal: read_yaml_value(yaml, "terminal".to_owned()),
            opener: read_yaml_value(yaml, "opener".to_owned()),
        }
    }

    fn is_complete(&self) -> bool {
        self.colors.is_complete()
            && self.keybindings.is_complete()
            && self.terminal.is_some()
            && self.opener.is_some()
    }

    fn update(&mut self, other: Self) {
        self.colors.update(other.colors);
        self.keybindings.update(other.keybindings);
        if other.terminal.is_some() {
            self.terminal = other.terminal
        }
        if other.opener.is_some() {
            self.opener = other.opener
        }
    }
}

// #[derive(Debug, Clone)]
// pub struct Colors {
//     pub file: String,
//     pub directory: String,
//     pub block: String,
//     pub char: String,
//     pub fifo: String,
//     pub socket: String,
//     pub symlink: String,
// }
//
// impl Colors {
//     pub fn from_config(yaml: &serde_yaml::value::Value) -> Self {
//         let file = yaml["file"]
//             .as_str()
//             .map(|s| s.to_string())
//             .expect("Couldn't parse config file");
//         let directory = yaml["directory"]
//             .as_str()
//             .map(|s| s.to_string())
//             .expect("Couldn't parse config file");
//         let block = yaml["block"]
//             .as_str()
//             .map(|s| s.to_string())
//             .expect("Couldn't parse config file");
//         let char = yaml["char"]
//             .as_str()
//             .map(|s| s.to_string())
//             .expect("Couldn't parse config file");
//         let fifo = yaml["fifo"]
//             .as_str()
//             .map(|s| s.to_string())
//             .expect("Couldn't parse config file");
//         let socket = yaml["socket"]
//             .as_str()
//             .map(|s| s.to_string())
//             .expect("Couldn't parse config file");
//         let symlink = yaml["symlink"]
//             .as_str()
//             .map(|s| s.to_string())
//             .expect("Couldn't parse config file");
//         Self {
//             file,
//             directory,
//             block,
//             char,
//             fifo,
//             socket,
//             symlink,
//         }
//     }
// }
//
// #[derive(Debug, Clone)]
// pub struct Keybindings {
//     pub toggle_hidden: char,
//     pub copy_paste: char,
//     pub cut_paste: char,
//     pub delete: char,
//     pub chmod: char,
//     pub exec: char,
//     pub newdir: char,
//     pub newfile: char,
//     pub rename: char,
//     pub clear_flags: char,
//     pub toggle_flag: char,
//     pub shell: char,
//     pub open_file: char,
//     pub help: char,
//     pub search: char,
//     pub quit: char,
//     pub goto: char,
//     pub flag_all: char,
//     pub reverse_flags: char,
//     pub regex_match: char,
//     pub jump: char,
// }
//
// impl Keybindings {
//     pub fn new(yaml: &serde_yaml::value::Value) -> Self {
//         let toggle_hidden = yaml["toggle_hidden"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "a".to_string())
//             .chars()
//             .next()
//             .unwrap_or('a');
//         let copy_paste = yaml["copy_paste"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "c".to_string())
//             .chars()
//             .next()
//             .unwrap_or('c');
//         let cut_paste = yaml["cut_paste"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "p".to_string())
//             .chars()
//             .next()
//             .unwrap_or('p');
//         let delete = yaml["delete"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "x".to_string())
//             .chars()
//             .next()
//             .unwrap_or('x');
//         let chmod = yaml["chmod"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "m".to_string())
//             .chars()
//             .next()
//             .unwrap_or('m');
//         let exec = yaml["exec"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "e".to_string())
//             .chars()
//             .next()
//             .unwrap_or('e');
//         let newdir = yaml["newdir"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "d".to_string())
//             .chars()
//             .next()
//             .unwrap_or('d');
//         let newfile = yaml["newfile"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "n".to_string())
//             .chars()
//             .next()
//             .unwrap_or('n');
//         let rename = yaml["rename"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "r".to_string())
//             .chars()
//             .next()
//             .unwrap_or('r');
//         let clear_flags = yaml["clear_flags"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "u".to_string())
//             .chars()
//             .next()
//             .unwrap_or('u');
//         let toggle_flag = yaml["toggle_flag"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| " ".to_string())
//             .chars()
//             .next()
//             .unwrap_or(' ');
//         let shell = yaml["shell"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "s".to_string())
//             .chars()
//             .next()
//             .unwrap_or('s');
//         let open_file = yaml["open_file"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "o".to_string())
//             .chars()
//             .next()
//             .unwrap_or('o');
//         let help = yaml["help"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "h".to_string())
//             .chars()
//             .next()
//             .unwrap_or('h');
//         let search = yaml["search"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "/".to_string())
//             .chars()
//             .next()
//             .unwrap_or('/');
//         let quit = yaml["quit"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "q".to_string())
//             .chars()
//             .next()
//             .unwrap_or('q');
//         let goto = yaml["goto"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "g".to_string())
//             .chars()
//             .next()
//             .unwrap_or('g');
//         let flag_all = yaml["flag_all"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "*".to_string())
//             .chars()
//             .next()
//             .unwrap_or('*');
//         let reverse_flags = yaml["reverse_flags"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "v".to_string())
//             .chars()
//             .next()
//             .unwrap_or('v');
//         let regex_match = yaml["regex_match"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "w".to_string())
//             .chars()
//             .next()
//             .unwrap_or('w');
//         let jump = yaml["jump"]
//             .as_str()
//             .map(|s| s.to_string())
//             .unwrap_or_else(|| "j".to_string())
//             .chars()
//             .next()
//             .unwrap_or('j');
//         Self {
//             toggle_hidden,
//             copy_paste,
//             cut_paste,
//             delete,
//             chmod,
//             exec,
//             newdir,
//             newfile,
//             rename,
//             clear_flags,
//             toggle_flag,
//             shell,
//             open_file,
//             help,
//             search,
//             quit,
//             goto,
//             flag_all,
//             reverse_flags,
//             regex_match,
//             jump,
//         }
//     }
// }

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

fn create_path(path_string: String) -> path::PathBuf {
    let expanded_string = shellexpand::tilde(&path_string).to_string();
    let mut expanded_path = path::PathBuf::new();
    expanded_path.push(expanded_string);
    expanded_path
}

pub fn read_config() -> OConfig {
    let user_path = create_path(HOME_CONFIG_DIR.to_owned());
    let etc_config_path = create_path(ETC_CONFIG_DIR.to_owned());
    let dev_path = create_path(std::env::var("CONFIG_FILE").unwrap_or("".into()));
    let mut cur_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    cur_path.push(CONFIG_NAME);
    let file = File::open(cur_path).unwrap();
    let mut oconfig = OConfig::new(&serde_yaml::from_reader(file).unwrap());

    for mut path in [dev_path, etc_config_path, user_path] {
        path.push(CONFIG_NAME);
        if let Ok(file) = File::open(path) {
            if let Ok(yaml) = serde_yaml::from_reader(file) {
                oconfig.update(OConfig::new(&yaml));
            }
        }
    }
    eprintln!("oconfig {:?}", oconfig);
    assert!(oconfig.is_complete());
    oconfig
}

// pub fn search_config() -> serde_yaml::Value {
//     read_config();
//     let user_path = create_path(HOME_CONFIG_DIR.to_owned());
//     eprintln!("user config path {:?}", user_path);
//
//     let etc_config_path = create_path(ETC_CONFIG_DIR.to_owned());
//     eprintln!("etc config path {:?}", etc_config_path);
//
//     let dev_path = create_path(std::env::var("CONFIG_FILE").unwrap_or("".into()));
//     eprintln!("dev config path {:?}", dev_path);
//
//     // let cur_path = get_current_working_dir().unwrap();
//     let cur_path = std::env::current_exe()
//         .unwrap()
//         .parent()
//         .unwrap()
//         .parent()
//         .unwrap()
//         .parent()
//         .unwrap()
//         .to_path_buf();
//     eprintln!("current exe in {:?}", cur_path);
//
//     for mut path in [dev_path] {
//         // for mut path in [user_path, etc_config_path, dev_path, cur_path] {
//         path.push(CONFIG_NAME);
//         eprintln!("trying {:?}", &path);
//         if let Ok(file) = File::open(path) {
//             if let Ok(yaml) = serde_yaml::from_reader(file) {
//                 let oconfig = OConfig::new(&yaml);
//                 eprintln!("oconfig {:?}", oconfig);
//                 assert!(oconfig.is_complete());
//                 return yaml;
//             }
//         }
//     }
//     std::process::exit(2);
// }
