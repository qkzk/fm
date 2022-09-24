use std::cmp::min;
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
use fm::fileinfo::{FileInfo, PathContent};

pub mod fileinfo;

const WINDOW_PADDING: usize = 4;
const WINDOW_MARGIN_TOP: usize = 1;
const EDIT_BOX_OFFSET: usize = 10;

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

    fn scroll_up_to(&mut self, row: usize) {
        if row < self.top + WINDOW_PADDING && self.top > 0 {
            self.top -= 1;
            self.bottom -= 1;
        }
    }

    fn scroll_down_to(&mut self, row: usize) {
        if self.len < self.height {
            return;
        }
        if row > self.bottom - WINDOW_PADDING && self.bottom < self.len - WINDOW_MARGIN_TOP {
            self.top += 1;
            self.bottom += 1;
        }
    }

    fn reset(&mut self, len: usize) {
        self.len = len;
        self.top = 0;
        self.bottom = min(len, self.height);
    }
}

fn fileinfo_attr(fileinfo: &FileInfo) -> Attr {
    let mut attr = Attr {
        fg: Color::WHITE,
        bg: Color::default(),
        effect: Effect::empty(),
    };
    if fileinfo.is_dir {
        attr.fg = Color::RED;
    } else if fileinfo.is_block {
        attr.fg = Color::YELLOW;
    } else if fileinfo.is_char {
        attr.fg = Color::MAGENTA;
    } else if fileinfo.is_fifo {
        attr.fg = Color::BLUE;
    } else if fileinfo.is_socket {
        attr.fg = Color::CYAN;
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
}

impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Mode::Normal => write!(f, "Normal:  "),
            Mode::Rename => write!(f, "Rename:  "),
            Mode::Chmod => write!(f, "Chmod:   "),
            Mode::Newfile => write!(f, "Newfile: "),
            Mode::Newdir => write!(f, "Newdir:  "),
        }
    }
}
// use fork::{daemon, Fork};
// use std::io::Result;

// fn run<I, S>(cmd: String, args: I)
// where
//     I: IntoIterator<Item = S>,
//     S: AsRef<std::ffi::OsStr>,
// {
//     if let Ok(Fork::Child) = daemon(false, false) {
//         Command::new(&cmd)
//             .args(args)
//             .output()
//             .expect("failed to execute process");
//     }
// }

pub fn execute_in_child(exe: &str, args: &[&str]) -> std::process::Child {
    Command::new(exe).args(args).spawn().unwrap()
}

fn main() {
    let mut config = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });
    let mut mode = Mode::Normal;
    let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    let (_, height) = term.term_size().unwrap();

    let path = std::fs::canonicalize(path::Path::new(&config.path)).unwrap();
    let mut path_content = PathContent::new(path, config.hidden);
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
                    window.scroll_up_to(file_index);
                }
            }
            Event::Key(Key::Down) => {
                if let Mode::Normal = mode {
                    if file_index < path_content.files.len() - WINDOW_MARGIN_TOP {
                        file_index += 1;
                    }
                    path_content.select_next();
                    window.scroll_down_to(file_index);
                }
            }
            Event::Key(Key::Left) => match mode {
                Mode::Normal => match path_content.path.parent() {
                    Some(parent) => {
                        path_content = PathContent::new(path::PathBuf::from(parent), config.hidden);
                        window.reset(path_content.files.len());
                        flagged.clear();
                        file_index = 0;
                        col = 0;
                    }
                    None => (),
                },
                Mode::Rename | Mode::Chmod | Mode::Newdir | Mode::Newfile => {
                    if col > 0 {
                        col -= 1
                    }
                }
            },

            Event::Key(Key::Right) => match mode {
                Mode::Normal => {
                    if path_content.files[path_content.selected].is_dir {
                        path_content = PathContent::new(
                            path_content.files[path_content.selected].path.clone(),
                            config.hidden,
                        );
                        window.reset(path_content.files.len());
                        flagged.clear();
                        file_index = 0;
                        col = 0;
                    } else {
                        execute_in_child(
                            "xdg-open",
                            &[path_content.files[path_content.selected]
                                .path
                                .to_str()
                                .unwrap()],
                        );
                    }
                }
                Mode::Rename | Mode::Chmod | Mode::Newdir | Mode::Newfile => {
                    if col < input_string.len() {
                        col += 1
                    }
                }
            },
            Event::Key(Key::Backspace) => match mode {
                Mode::Rename | Mode::Newdir | Mode::Chmod | Mode::Newfile => {
                    if col > 0 && !input_string.is_empty() {
                        input_string.remove(col - 1);
                        col -= 1;
                    }
                }
                Mode::Normal => (),
            },
            Event::Key(Key::Char(c)) => match mode {
                Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename => {
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
                        window.scroll_down_to(file_index);
                    }
                    'a' => {
                        config.hidden = !config.hidden;
                        path_content.show_hidden = !path_content.show_hidden;
                        path_content.reset_files();
                        window.reset(path_content.files.len())
                    }
                    'd' => mode = Mode::Newdir,
                    'm' => mode = Mode::Chmod,
                    'n' => mode = Mode::Newfile,
                    'r' => {
                        mode = Mode::Rename;
                        let oldname = path_content.files[path_content.selected].filename.clone();
                        oldpath = path_content.path.to_path_buf();
                        oldpath.push(oldname);
                    }
                    'q' => break,
                    's' => {
                        execute_in_child("st", &["-d", path_content.path.to_str().unwrap()]);
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
                    _ => (),
                },
            },
            Event::Key(Key::Enter) => {
                if let Mode::Rename = mode {
                    fs::rename(
                        oldpath.clone(),
                        path_content.path.to_path_buf().join(&input_string),
                    )
                    .unwrap_or(());
                    input_string.clear();
                    path_content = PathContent::new(path_content.path, config.hidden);
                    window.reset(path_content.files.len());
                } else if let Mode::Newfile = mode {
                    if fs::File::create(path_content.path.join(input_string.clone())).is_ok() {}
                    input_string.clear();
                    path_content = PathContent::new(path_content.path, config.hidden);
                    window.reset(path_content.files.len());
                } else if let Mode::Newdir = mode {
                    fs::create_dir(path_content.path.join(input_string.clone())).unwrap_or(());
                    input_string.clear();
                    path_content = PathContent::new(path_content.path, config.hidden);
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
                    path_content = PathContent::new(path_content.path, config.hidden);
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
                    config,
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
            let mut attr = fileinfo_attr(&path_content.files[i]);
            if flagged.contains(&path_content.files[i].path) {
                attr.effect |= Effect::UNDERLINE;
            }
            let _ = term.print_with_attr(row, 0, string, attr);
        }
        if let Mode::Normal = mode {
            let _ = term.set_cursor(0, 0);
        } else {
            let _ = term.set_cursor(0, col + EDIT_BOX_OFFSET);
        }

        let _ = term.present();
    }
}
