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

struct Status {
    term: Term<()>,
    mode: Mode,
    file_index: usize,
    window: FilesWindow,
    oldpath: path::PathBuf,
    flagged: HashSet<path::PathBuf>,
    input_string: String,
    col: usize,
    path_content: PathContent,
    height: usize,
    args: Config,
    colors: Colors,
}

impl Status {
    fn new(term: Term<()>, path_content: PathContent, args: Config, colors: Colors) -> Self {
        let mode = Mode::Normal;
        let (_, height) = term.term_size().unwrap();
        let file_index = 0;
        let window = FilesWindow::new(path_content.files.len(), height);
        let oldpath: path::PathBuf = path::PathBuf::new();
        let flagged = HashSet::new();
        let input_string = "".to_string();
        let col = 0;
        Self {
            term,
            mode,
            file_index,
            window,
            oldpath,
            flagged,
            input_string,
            col,
            path_content,
            height,
            args,
            colors,
        }
    }

    fn display_first_line(&mut self) {
        let first_row: String = match self.mode {
            Mode::Normal => {
                format!(
                    "h: {}, s: {} wt: {} wb: {}  m: {:?} - c: {:?} - {}",
                    self.height,
                    self.path_content.files.len(),
                    self.window.top,
                    self.window.bottom,
                    self.mode,
                    self.args,
                    self.path_content.path.to_str().unwrap()
                )
            }
            _ => {
                format!("{:?} {}", self.mode.clone(), self.input_string.clone())
            }
        };
        let _ = self.term.print(0, 0, &first_row);
    }

    fn display_files(&mut self) {
        let strings = self.path_content.strings();
        for (i, string) in strings
            .iter()
            .enumerate()
            .take(min(strings.len(), self.window.bottom))
            .skip(self.window.top)
        {
            let row = i + WINDOW_MARGIN_TOP - self.window.top;
            let mut attr = fileinfo_attr(&self.path_content.files[i], &self.colors);
            if self.flagged.contains(&self.path_content.files[i].path) {
                attr.effect |= Effect::UNDERLINE;
            }
            let _ = self.term.print_with_attr(row, 0, string, attr);
        }
    }

    fn display_help_or_cursor(&mut self) {
        match self.mode {
            Mode::Normal => {
                let _ = self.term.set_cursor(0, 0);
            }
            Mode::Help => {
                let _ = self.term.clear();
                for (row, line) in HELP_LINES.split('\n').enumerate() {
                    let _ = self.term.print(row, 0, line);
                }
            }
            _ => {
                let _ = self.term.set_cursor(0, self.col + EDIT_BOX_OFFSET);
            }
        }
    }
}

fn main() {
    let args = Config::new(env::args()).unwrap_or_else(|err| {
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
    let path_content = PathContent::new(path, args.hidden);

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
    let mut status = Status::new(term, path_content, args, colors);

    while let Ok(ev) = status.term.poll_event() {
        let _ = status.term.clear();
        let (_width, height) = status.term.term_size().unwrap();
        status.window.height = height;
        match ev {
            Event::Key(Key::ESC) => {
                status.input_string.clear();
                status.path_content.reset_files();
                status.window.reset(status.path_content.files.len());
                status.mode = Mode::Normal;
                status.col = 0;
            }
            Event::Key(Key::Up) => {
                if let Mode::Normal = status.mode {
                    if status.file_index > 0 {
                        status.file_index -= 1;
                    }
                    status.path_content.select_prev();
                    status.window.scroll_up_one(status.file_index);
                }
            }
            Event::Key(Key::Down) => {
                if let Mode::Normal = status.mode {
                    if status.file_index < status.path_content.files.len() - WINDOW_MARGIN_TOP {
                        status.file_index += 1;
                    }
                    status.path_content.select_next();
                    status.window.scroll_down_one(status.file_index);
                }
            }
            Event::Key(Key::Left) => match status.mode {
                Mode::Normal => match status.path_content.path.parent() {
                    Some(parent) => {
                        status.path_content =
                            PathContent::new(path::PathBuf::from(parent), status.args.hidden);
                        status.window.reset(status.path_content.files.len());
                        status.file_index = 0;
                        status.col = 0;
                    }
                    None => (),
                },
                Mode::Rename
                | Mode::Chmod
                | Mode::Newdir
                | Mode::Newfile
                | Mode::Exec
                | Mode::Search => {
                    if status.col > 0 {
                        status.col -= 1
                    }
                }
                _ => (),
            },

            Event::Key(Key::Right) => match status.mode {
                Mode::Normal => {
                    if status.path_content.files[status.path_content.selected].is_dir {
                        status.path_content = PathContent::new(
                            status.path_content.files[status.path_content.selected]
                                .path
                                .clone(),
                            status.args.hidden,
                        );
                        status.window.reset(status.path_content.files.len());
                        status.file_index = 0;
                        status.col = 0;
                    }
                }
                Mode::Rename
                | Mode::Chmod
                | Mode::Newdir
                | Mode::Newfile
                | Mode::Exec
                | Mode::Search => {
                    if status.col < status.input_string.len() {
                        status.col += 1
                    }
                }
                _ => (),
            },
            Event::Key(Key::Backspace) => match status.mode {
                Mode::Rename
                | Mode::Newdir
                | Mode::Chmod
                | Mode::Newfile
                | Mode::Exec
                | Mode::Search => {
                    if status.col > 0 && !status.input_string.is_empty() {
                        status.input_string.remove(status.col - 1);
                        status.col -= 1;
                    }
                }
                Mode::Normal => (),
                _ => (),
            },
            Event::Key(Key::Char(c)) => match status.mode {
                Mode::Newfile
                | Mode::Newdir
                | Mode::Chmod
                | Mode::Rename
                | Mode::Exec
                | Mode::Search => {
                    status.input_string.insert(status.col, c);
                    status.col += 1;
                }
                Mode::Normal => match c {
                    ' ' => {
                        if status
                            .flagged
                            .contains(&status.path_content.files[status.file_index].path)
                        {
                            status
                                .flagged
                                .remove(&status.path_content.files[status.file_index].path);
                        } else {
                            status
                                .flagged
                                .insert(status.path_content.files[status.file_index].path.clone());
                        }
                        if status.file_index < status.path_content.files.len() - WINDOW_MARGIN_TOP {
                            status.file_index += 1;
                        }
                        status.path_content.select_next();
                        status.window.scroll_down_one(status.file_index);
                    }
                    'a' => {
                        status.args.hidden = !status.args.hidden;
                        status.path_content.show_hidden = !status.path_content.show_hidden;
                        status.path_content.reset_files();
                        status.window.reset(status.path_content.files.len())
                    }
                    'c' => {
                        status.flagged.iter().for_each(|oldpath| {
                            let newpath = status
                                .path_content
                                .path
                                .clone()
                                .join(oldpath.as_path().file_name().unwrap());
                            fs::copy(oldpath, newpath).unwrap_or(0);
                        });
                        status.flagged.clear();
                        status.path_content.reset_files();
                        status.window.reset(status.path_content.files.len());
                    }
                    'd' => status.mode = Mode::Newdir,
                    'e' => status.mode = Mode::Exec,
                    'm' => status.mode = Mode::Chmod,
                    'n' => status.mode = Mode::Newfile,
                    'o' => {
                        execute_in_child(
                            &opener,
                            &vec![status.path_content.files[status.path_content.selected]
                                .path
                                .to_str()
                                .unwrap()],
                        );
                    }
                    'p' => {
                        status.flagged.iter().for_each(|oldpath| {
                            let newpath = status
                                .path_content
                                .path
                                .clone()
                                .join(oldpath.as_path().file_name().unwrap());
                            fs::rename(oldpath, newpath).unwrap_or(());
                        });
                        status.flagged.clear();
                        status.path_content.reset_files();
                        status.window.reset(status.path_content.files.len());
                    }
                    'r' => {
                        status.mode = Mode::Rename;
                        let oldname = status.path_content.files[status.path_content.selected]
                            .filename
                            .clone();
                        status.oldpath = status.path_content.path.to_path_buf();
                        status.oldpath.push(oldname);
                    }
                    'q' => break,
                    's' => {
                        execute_in_child(
                            &terminal,
                            &vec!["-d", status.path_content.path.to_str().unwrap()],
                        );
                    }
                    'u' => status.flagged.clear(),
                    'x' => {
                        status.flagged.iter().for_each(|pathbuf| {
                            if pathbuf.is_dir() {
                                fs::remove_dir_all(pathbuf).unwrap_or(());
                            } else {
                                fs::remove_file(pathbuf).unwrap_or(());
                            }
                        });
                        status.flagged.clear();
                        status.path_content.reset_files();
                        status.window.reset(status.path_content.files.len());
                    }
                    '?' => status.mode = Mode::Help,
                    '/' => status.mode = Mode::Search,
                    _ => (),
                },
                Mode::Help => {
                    if c == '?' {
                        status.mode = Mode::Normal
                    } else if c == 'q' {
                        break;
                    }
                }
            },
            Event::Key(Key::Home) => {
                if let Mode::Normal = status.mode {
                    status.path_content.select_index(0);
                    status.file_index = 0;
                    status.window.scroll_to(0);
                }
            }
            Event::Key(Key::End) => {
                if let Mode::Normal = status.mode {
                    let last_index = status.path_content.files.len() - 1;
                    status.path_content.select_index(last_index);
                    status.file_index = last_index;
                    status.window.scroll_to(last_index);
                }
            }
            Event::Key(Key::PageDown) => {
                if let Mode::Normal = status.mode {
                    let down_index =
                        min(status.path_content.files.len() - 1, status.file_index + 10);
                    status.path_content.select_index(down_index);
                    status.file_index = down_index;
                    status.window.scroll_to(down_index);
                }
            }
            Event::Key(Key::PageUp) => {
                if let Mode::Normal = status.mode {
                    let up_index = if status.file_index > 10 {
                        status.file_index - 10
                    } else {
                        0
                    };
                    status.path_content.select_index(up_index);
                    status.file_index = up_index;
                    status.window.scroll_to(up_index);
                }
            }

            Event::Key(Key::Enter) => {
                if let Mode::Rename = status.mode {
                    fs::rename(
                        status.oldpath.clone(),
                        status
                            .path_content
                            .path
                            .to_path_buf()
                            .join(&status.input_string),
                    )
                    .unwrap_or(());
                    status.input_string.clear();
                    status.path_content =
                        PathContent::new(status.path_content.path, status.args.hidden);
                    status.window.reset(status.path_content.files.len());
                } else if let Mode::Newfile = status.mode {
                    if fs::File::create(status.path_content.path.join(status.input_string.clone()))
                        .is_ok()
                    {}
                    status.input_string.clear();
                    status.path_content =
                        PathContent::new(status.path_content.path, status.args.hidden);
                    status.window.reset(status.path_content.files.len());
                } else if let Mode::Newdir = status.mode {
                    fs::create_dir(status.path_content.path.join(status.input_string.clone()))
                        .unwrap_or(());
                    status.input_string.clear();
                    status.path_content =
                        PathContent::new(status.path_content.path, status.args.hidden);
                    status.window.reset(status.path_content.files.len());
                } else if let Mode::Chmod = status.mode {
                    let permissions: u32 =
                        u32::from_str_radix(&status.input_string, 8).unwrap_or(0_u32);
                    if permissions <= 0o777 {
                        fs::set_permissions(
                            status.path_content.files[status.file_index].path.clone(),
                            fs::Permissions::from_mode(permissions),
                        )
                        .unwrap_or(());
                    }
                    status.input_string.clear();
                    status.path_content =
                        PathContent::new(status.path_content.path, status.args.hidden);
                } else if let Mode::Exec = status.mode {
                    let exec_command = status.input_string.clone();
                    let mut args: Vec<&str> = exec_command.split(' ').collect();
                    let command = args.remove(0);
                    args.push(
                        status.path_content.files[status.path_content.selected]
                            .path
                            .to_str()
                            .unwrap(),
                    );
                    status.input_string.clear();
                    execute_in_child(command, &args);
                } else if let Mode::Search = status.mode {
                    let searched_term = status.input_string.clone();
                    let mut next_index = status.file_index;
                    for (index, file) in status
                        .path_content
                        .files
                        .iter()
                        .enumerate()
                        .skip(next_index)
                    {
                        if file.filename.contains(&searched_term) {
                            next_index = index;
                            break;
                        };
                    }
                    status.input_string.clear();
                    status.path_content.select_index(next_index);
                    status.file_index = next_index;
                    status.window.scroll_to(status.file_index);
                }

                status.col = 0;
                status.mode = Mode::Normal;
            }
            _ => {}
        }

        status.display_first_line();
        status.display_files();
        status.display_help_or_cursor();

        let _ = status.term.present();
    }
}
