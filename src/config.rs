use std::{fs::File, path};

use serde_yaml;
use tuikit::attr::Color;

/// Holds every configurable aspect of the application.
/// All attributes are hardcoded then updated from optional values
/// of the config file.
/// The config file is a YAML file in `~/.config/fm/config.yaml`
/// Default config file looks like this and is easier to read in code directly.
/// # the terminal must be installed
/// terminal: st
/// # the opener must be installed
/// opener: rifle
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
#[derive(Debug, Clone)]
pub struct Config {
    /// Color of every kind of file
    pub colors: Colors,
    /// Configurable keybindings.
    pub keybindings: Keybindings,
    /// Terminal used to open file
    pub terminal: String,
    /// File opener
    pub opener: String,
}

impl Config {
    /// Returns a default config with hardcoded values.
    fn new() -> Self {
        Self {
            colors: Colors::default(),
            keybindings: Keybindings::default(),
            terminal: "st".to_owned(),
            opener: "xdg-open".to_owned(),
        }
    }

    /// Updates the config from  a configuration content.
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        self.colors.update_from_config(&yaml["colors"]);
        self.keybindings.update_from_config(&yaml["keybindings"]);
        if let Some(terminal) = yaml["terminal"].as_str().map(|s| s.to_string()) {
            self.terminal = terminal;
        }
        if let Some(opener) = yaml["opener"].as_str().map(|s| s.to_string()) {
            self.opener = opener;
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

#[derive(Debug, Clone)]
pub struct Keybindings {
    /// Toggling the display between hidden and not
    pub toggle_hidden: char,
    /// Copy and paste the currently flagged files
    pub copy_paste: char,
    /// Cut and paste (ie move) the currently flagged files
    pub cut_paste: char,
    /// Delete the currently flagged files
    pub delete: char,
    /// Edit the permissions of selected file.
    pub chmod: char,
    /// Exec a command on the selected file.
    pub exec: char,
    /// Creates a new directory.
    pub newdir: char,
    /// Creates a new file.
    pub newfile: char,
    /// Rename the selected file.
    pub rename: char,
    /// Empty the flagged files
    pub clear_flags: char,
    /// Toggle all the selected file as flag and move to next file.
    pub toggle_flag: char,
    /// Open a terminal in current directory
    pub shell: char,
    /// Open a file with configured file opener
    pub open_file: char,
    /// Display the help with default keybindings
    pub help: char,
    /// Search for a file
    pub search: char,
    /// Quit the application
    pub quit: char,
    /// Move to a directory
    pub goto: char,
    /// Set all files as flagged in current directory
    pub flag_all: char,
    /// Reverse all the flagged in current directory
    pub reverse_flags: char,
    /// Flag all files matching a regex
    pub regex_match: char,
    /// Jump to a flagged directory.
    pub jump: char,
    /// Pick the current file in NVIM.
    /// The application must have been launched from NVIM for it to work.
    pub nvim: char,
    /// Change file sorting.
    pub sort_by: char,
    /// Creates asymlink
    pub symlink: char,
}

impl Keybindings {
    /// Updates the keybindings from a YAML content.
    /// If a bind isn't present, the old value is kept.
    fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        if let Some(toggle_hidden) = yaml["toggle_hidden"].as_str().map(|s| s.to_string()) {
            self.toggle_hidden = toggle_hidden.chars().next().unwrap_or('a');
        }
        if let Some(copy_paste) = yaml["copy_paste"].as_str().map(|s| s.to_string()) {
            self.copy_paste = copy_paste.chars().next().unwrap_or('c');
        }
        if let Some(cut_paste) = yaml["cut_paste"].as_str().map(|s| s.to_string()) {
            self.cut_paste = cut_paste.chars().next().unwrap_or('p');
        }
        if let Some(delete) = yaml["delete"].as_str().map(|s| s.to_string()) {
            self.delete = delete.chars().next().unwrap_or('x');
        }
        if let Some(chmod) = yaml["chmod"].as_str().map(|s| s.to_string()) {
            self.chmod = chmod.chars().next().unwrap_or('m');
        }
        if let Some(exec) = yaml["exec"].as_str().map(|s| s.to_string()) {
            self.exec = exec.chars().next().unwrap_or('e');
        }
        if let Some(newdir) = yaml["newdir"].as_str().map(|s| s.to_string()) {
            self.newdir = newdir.chars().next().unwrap_or('d');
        }
        if let Some(newfile) = yaml["newfile"].as_str().map(|s| s.to_string()) {
            self.newfile = newfile.chars().next().unwrap_or('n');
        }
        if let Some(rename) = yaml["rename"].as_str().map(|s| s.to_string()) {
            self.rename = rename.chars().next().unwrap_or('r');
        }
        if let Some(clear_flags) = yaml["clear_flags"].as_str().map(|s| s.to_string()) {
            self.clear_flags = clear_flags.chars().next().unwrap_or('u');
        }
        if let Some(toggle_flag) = yaml["toggle_flag"].as_str().map(|s| s.to_string()) {
            self.toggle_flag = toggle_flag.chars().next().unwrap_or(' ');
        }
        if let Some(shell) = yaml["shell"].as_str().map(|s| s.to_string()) {
            self.shell = shell.chars().next().unwrap_or('s');
        }
        if let Some(open_file) = yaml["open_file"].as_str().map(|s| s.to_string()) {
            self.open_file = open_file.chars().next().unwrap_or('o');
        }
        if let Some(help) = yaml["help"].as_str().map(|s| s.to_string()) {
            self.help = help.chars().next().unwrap_or('h');
        }
        if let Some(search) = yaml["search"].as_str().map(|s| s.to_string()) {
            self.search = search.chars().next().unwrap_or('/');
        }
        if let Some(quit) = yaml["quit"].as_str().map(|s| s.to_string()) {
            self.quit = quit.chars().next().unwrap_or('q');
        }
        if let Some(goto) = yaml["goto"].as_str().map(|s| s.to_string()) {
            self.goto = goto.chars().next().unwrap_or('g');
        }
        if let Some(flag_all) = yaml["flag_all"].as_str().map(|s| s.to_string()) {
            self.flag_all = flag_all.chars().next().unwrap_or('*');
        }
        if let Some(reverse_flags) = yaml["reverse_flags"].as_str().map(|s| s.to_string()) {
            self.reverse_flags = reverse_flags.chars().next().unwrap_or('v');
        }
        if let Some(regex_match) = yaml["regex_match"].as_str().map(|s| s.to_string()) {
            self.regex_match = regex_match.chars().next().unwrap_or('w');
        }
        if let Some(jump) = yaml["jump"].as_str().map(|s| s.to_string()) {
            self.jump = jump.chars().next().unwrap_or('j');
        }
        if let Some(nvim) = yaml["nvim"].as_str().map(|s| s.to_string()) {
            self.nvim = nvim.chars().next().unwrap_or('i');
        }
        if let Some(sort_by) = yaml["sort_by"].as_str().map(|s| s.to_string()) {
            self.sort_by = sort_by.chars().next().unwrap_or('O');
        }
        if let Some(symlink) = yaml["symblink"].as_str().map(|s| s.to_string()) {
            self.symlink = symlink.chars().next().unwrap_or('S');
        }
    }

    /// Returns a new `Keybindings` instance with hardcoded values.
    fn new() -> Self {
        Self {
            toggle_hidden: 'a',
            copy_paste: 'c',
            cut_paste: 'p',
            delete: 'x',
            chmod: 'm',
            exec: 'e',
            newdir: 'd',
            newfile: 'n',
            rename: 'r',
            clear_flags: 'u',
            toggle_flag: ' ',
            shell: 's',
            open_file: 'o',
            help: 'h',
            search: '/',
            quit: 'q',
            goto: 'g',
            flag_all: '*',
            reverse_flags: 'v',
            regex_match: 'w',
            jump: 'j',
            nvim: 'i',
            sort_by: 'O',
            symlink: 'S',
        }
    }
}

impl Default for Keybindings {
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
