use std::cmp::{max, min};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::{env, path, process};

use tuikit::attr::*;
use tuikit::event::{Event, Key};
use tuikit::term::{Term, TermHeight};

use fm::config::Config;
use fm::config_file::{load_file, str_to_tuikit, Colors};
use fm::fileinfo::{FileInfo, PathContent};

pub mod fileinfo;

const WINDOW_PADDING: usize = 4;
const WINDOW_MARGIN_TOP: usize = 1;
const EDIT_BOX_OFFSET: usize = 10;
static CONFIG_FILE: &str = "/home/quentin/gclem/dev/rust/fm/config.yaml";
static USAGE: &str = "
FM: dired inspired File Manager

dired [flags] [path]
flags:
-a display hidden files
-h show help and exit
";
static HELP_LINES: &str = "
Default key bindings:

q:      quit
?:      help

- Navigation -
←:      cd to parent directory 
→:      cd to child directory
↑:      one line up  
↓:      one line down
Home:   go to first line
End:    go to last line
PgUp:   10 lines up
PgDown: 10 lines down

a:      toggle hidden
s:      shell in current directory
o:      xdg-open this file

- Action on flagged files - 
    space:  toggle flag on a file 
    u:      clear flags
    c:      copy to current dir
    p:      move to current dir
    x:      delete flagged files

- MODES - 
    m:      CHMOD 
    e:      EXEC 
    d:      NEWDIR 
    n:      NEWFILE
    r:      RENAME
    Enter:  Execute mode then NORMAL
    Esc:    NORMAL
";

struct FilesWindow {
    top: usize,
    bottom: usize,
    len: usize,
    height: usize,
}

impl FilesWindow {
    fn new(len: usize, height: usize) -> Self {
        FilesWindow {
            top: 0,
            bottom: min(len, height - 3),
            len,
            height: height - 3,
        }
    }

    fn scroll_up_one(&mut self, index: usize) {
        if index < self.top + WINDOW_PADDING && self.top > 0 {
            self.top -= 1;
            self.bottom -= 1;
        }
    }

    fn scroll_down_one(&mut self, index: usize) {
        if self.len < self.height {
            return;
        }
        if index > self.bottom - WINDOW_PADDING && self.bottom < self.len - WINDOW_MARGIN_TOP {
            self.top += 1;
            self.bottom += 1;
        }
    }

    fn reset(&mut self, len: usize) {
        self.len = len;
        self.top = 0;
        self.bottom = min(len, self.height);
    }

    fn scroll_to(&mut self, index: usize) {
        if index < self.top || index > self.bottom {
            self.top = max(index, WINDOW_PADDING) - WINDOW_PADDING;
            self.bottom = self.top + min(self.len, self.height - 3);
        }
    }
}

fn fileinfo_attr(fileinfo: &FileInfo, colors: &Colors) -> Attr {
    let mut attr = Attr {
        fg: str_to_tuikit(&colors.file),
        bg: Color::default(),
        effect: Effect::empty(),
    };
    if fileinfo.is_dir {
        attr.fg = str_to_tuikit(&colors.directory);
    } else if fileinfo.is_block {
        attr.fg = str_to_tuikit(&colors.block);
    } else if fileinfo.is_char {
        attr.fg = str_to_tuikit(&colors.char)
    } else if fileinfo.is_fifo {
        attr.fg = str_to_tuikit(&colors.fifo);
    } else if fileinfo.is_socket {
        attr.fg = str_to_tuikit(&colors.socket);
    }
    if fileinfo.is_selected {
        attr.effect = Effect::REVERSE;
    }
    attr
}

#[derive(Clone)]
enum Mode {
    Normal,
    Rename,
    Chmod,
    Newfile,
    Newdir,
    Exec,
    Help,
    Search,
}

impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Mode::Normal => write!(f, "Normal:  "),
            Mode::Rename => write!(f, "Rename:  "),
            Mode::Chmod => write!(f, "Chmod:   "),
            Mode::Newfile => write!(f, "Newfile: "),
            Mode::Newdir => write!(f, "Newdir:  "),
            Mode::Exec => write!(f, "Exec:    "),
            Mode::Help => write!(f, ""),
            Mode::Search => write!(f, "Search:  "),
        }
    }
}

pub fn execute_in_child(exe: &str, args: &Vec<&str>) -> std::process::Child {
    Command::new(exe).args(args).spawn().unwrap()
}

fn help() {
    print!("{}", USAGE);
    print!("{}", HELP_LINES);
}

fn main() {
    let mut args = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        help();
        process::exit(1);
    });
    if args.help {
        help();
        process::exit(0);
    }
    let path = std::fs::canonicalize(path::Path::new(&args.path)).unwrap_or_else(|_| {
        eprintln!("File does not exists {}", args.path);
        std::process::exit(2)
    });
    let mut path_content = PathContent::new(path, args.hidden);

    let mut mode = Mode::Normal;
    let config_file = load_file(CONFIG_FILE);
    let colors = Colors::from_config(&config_file["colors"]);
    let terminal = config_file["terminal"]
        .as_str()
        .map(|s| s.to_string())
        .expect("Couldn't parse config file");
    let opener = config_file["opener"]
        .as_str()
        .map(|s| s.to_string())
        .expect("Couldn't parse config file");
    let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    let (_, height) = term.term_size().unwrap();

    let mut file_index = 0;
    let mut window = FilesWindow::new(path_content.files.len(), height);
    let mut oldpath: path::PathBuf = path::PathBuf::new();
    let mut flagged = HashSet::new();
    let mut input_string = "".to_string();
    let mut col = 0_usize;

    while let Ok(ev) = term.poll_event() {
        let _ = term.clear();
        let (_width, height) = term.term_size().unwrap();
        window.height = height;
        match ev {
            Event::Key(Key::ESC) => {
                input_string.clear();
                path_content.reset_files();
                window.reset(path_content.files.len());
                mode = Mode::Normal;
                col = 0;
            }
            Event::Key(Key::Up) => {
                if let Mode::Normal = mode {
                    if file_index > 0 {
                        file_index -= 1;
                    }
                    path_content.select_prev();
                    window.scroll_up_one(file_index);
                }
            }
            Event::Key(Key::Down) => {
                if let Mode::Normal = mode {
                    if file_index < path_content.files.len() - WINDOW_MARGIN_TOP {
                        file_index += 1;
                    }
                    path_content.select_next();
                    window.scroll_down_one(file_index);
                }
            }
            Event::Key(Key::Left) => match mode {
                Mode::Normal => match path_content.path.parent() {
                    Some(parent) => {
                        path_content = PathContent::new(path::PathBuf::from(parent), args.hidden);
                        window.reset(path_content.files.len());
                        // flagged.clear();
                        file_index = 0;
                        col = 0;
                    }
                    None => (),
                },
                Mode::Rename
                | Mode::Chmod
                | Mode::Newdir
                | Mode::Newfile
                | Mode::Exec
                | Mode::Search => {
                    if col > 0 {
                        col -= 1
                    }
                }
                _ => (),
            },

            Event::Key(Key::Right) => match mode {
                Mode::Normal => {
                    if path_content.files[path_content.selected].is_dir {
                        path_content = PathContent::new(
                            path_content.files[path_content.selected].path.clone(),
                            args.hidden,
                        );
                        window.reset(path_content.files.len());
                        // flagged.clear();
                        file_index = 0;
                        col = 0;
                    }
                }
                Mode::Rename
                | Mode::Chmod
                | Mode::Newdir
                | Mode::Newfile
                | Mode::Exec
                | Mode::Search => {
                    if col < input_string.len() {
                        col += 1
                    }
                }
                _ => (),
            },
            Event::Key(Key::Backspace) => match mode {
                Mode::Rename
                | Mode::Newdir
                | Mode::Chmod
                | Mode::Newfile
                | Mode::Exec
                | Mode::Search => {
                    if col > 0 && !input_string.is_empty() {
                        input_string.remove(col - 1);
                        col -= 1;
                    }
                }
                Mode::Normal => (),
                _ => (),
            },
            Event::Key(Key::Char(c)) => match mode {
                Mode::Newfile
                | Mode::Newdir
                | Mode::Chmod
                | Mode::Rename
                | Mode::Exec
                | Mode::Search => {
                    input_string.insert(col, c);
                    col += 1;
                }
                Mode::Normal => match c {
                    ' ' => {
                        if flagged.contains(&path_content.files[file_index].path) {
                            flagged.remove(&path_content.files[file_index].path);
                        } else {
                            flagged.insert(path_content.files[file_index].path.clone());
                        }
                        if file_index < path_content.files.len() - WINDOW_MARGIN_TOP {
                            file_index += 1;
                        }
                        path_content.select_next();
                        window.scroll_down_one(file_index);
                    }
                    'a' => {
                        args.hidden = !args.hidden;
                        path_content.show_hidden = !path_content.show_hidden;
                        path_content.reset_files();
                        window.reset(path_content.files.len())
                    }
                    'c' => {
                        flagged.iter().for_each(|oldpath| {
                            let newpath = path_content
                                .path
                                .clone()
                                .join(oldpath.as_path().file_name().unwrap());
                            fs::copy(oldpath, newpath).unwrap_or(0);
                        });
                        flagged.clear();
                        path_content.reset_files();
                        window.reset(path_content.files.len());
                    }
                    'd' => mode = Mode::Newdir,
                    'e' => mode = Mode::Exec,
                    'm' => mode = Mode::Chmod,
                    'n' => mode = Mode::Newfile,
                    'o' => {
                        execute_in_child(
                            &opener,
                            &vec![path_content.files[path_content.selected]
                                .path
                                .to_str()
                                .unwrap()],
                        );
                    }
                    'p' => {
                        flagged.iter().for_each(|oldpath| {
                            let newpath = path_content
                                .path
                                .clone()
                                .join(oldpath.as_path().file_name().unwrap());
                            fs::rename(oldpath, newpath).unwrap_or(());
                        });
                        flagged.clear();
                        path_content.reset_files();
                        window.reset(path_content.files.len());
                    }
                    'r' => {
                        mode = Mode::Rename;
                        let oldname = path_content.files[path_content.selected].filename.clone();
                        oldpath = path_content.path.to_path_buf();
                        oldpath.push(oldname);
                    }
                    'q' => break,
                    's' => {
                        execute_in_child(
                            &terminal,
                            &vec!["-d", path_content.path.to_str().unwrap()],
                        );
                    }
                    'u' => flagged.clear(),
                    'x' => {
                        flagged.iter().for_each(|pathbuf| {
                            if pathbuf.is_dir() {
                                fs::remove_dir_all(pathbuf).unwrap_or(());
                            } else {
                                fs::remove_file(pathbuf).unwrap_or(());
                            }
                        });
                        flagged.clear();
                        path_content.reset_files();
                        window.reset(path_content.files.len());
                    }
                    '?' => mode = Mode::Help,
                    '/' => mode = Mode::Search,
                    _ => (),
                },
                Mode::Help => {
                    if c == '?' {
                        mode = Mode::Normal
                    } else if c == 'q' {
                        break;
                    }
                }
            },
            Event::Key(Key::Home) => {
                if let Mode::Normal = mode {
                    path_content.select_index(0);
                    file_index = 0;
                    window.scroll_to(0);
                }
            }
            Event::Key(Key::End) => {
                if let Mode::Normal = mode {
                    let last_index = path_content.files.len() - 1;
                    path_content.select_index(last_index);
                    file_index = last_index;
                    window.scroll_to(last_index);
                }
            }
            Event::Key(Key::PageDown) => {
                if let Mode::Normal = mode {
                    let down_index = min(path_content.files.len() - 1, file_index + 10);
                    path_content.select_index(down_index);
                    file_index = down_index;
                    window.scroll_to(down_index);
                }
            }
            Event::Key(Key::PageUp) => {
                if let Mode::Normal = mode {
                    let up_index = if file_index > 10 { file_index - 10 } else { 0 };
                    path_content.select_index(up_index);
                    file_index = up_index;
                    window.scroll_to(up_index);
                }
            }

            Event::Key(Key::Enter) => {
                if let Mode::Rename = mode {
                    fs::rename(
                        oldpath.clone(),
                        path_content.path.to_path_buf().join(&input_string),
                    )
                    .unwrap_or(());
                    input_string.clear();
                    path_content = PathContent::new(path_content.path, args.hidden);
                    window.reset(path_content.files.len());
                } else if let Mode::Newfile = mode {
                    if fs::File::create(path_content.path.join(input_string.clone())).is_ok() {}
                    input_string.clear();
                    path_content = PathContent::new(path_content.path, args.hidden);
                    window.reset(path_content.files.len());
                } else if let Mode::Newdir = mode {
                    fs::create_dir(path_content.path.join(input_string.clone())).unwrap_or(());
                    input_string.clear();
                    path_content = PathContent::new(path_content.path, args.hidden);
                    window.reset(path_content.files.len());
                } else if let Mode::Chmod = mode {
                    let permissions: u32 = u32::from_str_radix(&input_string, 8).unwrap_or(0_u32);
                    if permissions <= 0o777 {
                        fs::set_permissions(
                            path_content.files[file_index].path.clone(),
                            fs::Permissions::from_mode(permissions),
                        )
                        .unwrap_or(());
                    }
                    input_string.clear();
                    path_content = PathContent::new(path_content.path, args.hidden);
                } else if let Mode::Exec = mode {
                    let exec_command = input_string.clone();
                    let mut args: Vec<&str> = exec_command.split(' ').collect();
                    let command = args.remove(0);
                    args.push(
                        path_content.files[path_content.selected]
                            .path
                            .to_str()
                            .unwrap(),
                    );
                    input_string.clear();
                    execute_in_child(command, &args);
                } else if let Mode::Search = mode {
                    let searched_term = input_string.clone();
                    let mut next_index = file_index;
                    for (index, file) in path_content.files.iter().enumerate().skip(next_index) {
                        if file.filename.contains(&searched_term) {
                            next_index = index;
                            break;
                        };
                    }
                    input_string.clear();
                    path_content.select_index(next_index);
                    file_index = next_index;
                    window.scroll_to(file_index);
                }

                col = 0;
                mode = Mode::Normal;
            }
            _ => {}
        }
        let first_row: String = match mode {
            Mode::Normal => {
                format!(
                    "h: {}, s: {} wt: {} wb: {}  m: {:?} - c: {:?} - {}",
                    height,
                    path_content.files.len(),
                    window.top,
                    window.bottom,
                    mode,
                    args,
                    path_content.path.to_str().unwrap()
                )
            }
            _ => {
                format!("{:?} {}", mode.clone(), input_string.clone())
            }
        };
        let _ = term.print(0, 0, &first_row);
        let strings = path_content.strings();
        for (i, string) in strings
            .iter()
            .enumerate()
            .take(min(strings.len(), window.bottom))
            .skip(window.top)
        {
            let row = i + WINDOW_MARGIN_TOP - window.top;
            let mut attr = fileinfo_attr(&path_content.files[i], &colors);
            if flagged.contains(&path_content.files[i].path) {
                attr.effect |= Effect::UNDERLINE;
            }
            let _ = term.print_with_attr(row, 0, string, attr);
        }
        match mode {
            Mode::Normal => {
                let _ = term.set_cursor(0, 0);
            }
            Mode::Help => {
                let _ = term.clear();
                for (row, line) in HELP_LINES.split('\n').enumerate() {
                    let _ = term.print(row, 0, line);
                }
            }
            _ => {
                let _ = term.set_cursor(0, col + EDIT_BOX_OFFSET);
            }
        }

        let _ = term.present();
    }
}
