use std::cmp::{max, min};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::{env, path, process};

use tuikit::attr::*;
use tuikit::event::{Event, Key};
use tuikit::key::MouseButton;
use tuikit::term::{Term, TermHeight};

use fm::config::Config;
use fm::config_file::{load_file, str_to_tuikit, Colors, Keybindings};
use fm::fileinfo::{FileInfo, PathContent};

pub mod fileinfo;

const WINDOW_PADDING: usize = 4;
const WINDOW_MARGIN_TOP: usize = 1;
const EDIT_BOX_OFFSET: usize = 10;
const MAX_PERMISSIONS: u32 = 0o777;
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
    g:      GOTO
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
    Goto,
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
            Mode::Goto => write!(f, "Goto  :  "),
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
    terminal: String,
    opener: String,
    keybindings: Keybindings,
}

impl Status {
    fn new(args: Config, height: usize) -> Self {
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
        let mode = Mode::Normal;
        let file_index = 0;
        let window = FilesWindow::new(path_content.files.len(), height);
        let oldpath: path::PathBuf = path::PathBuf::new();
        let flagged = HashSet::new();
        let input_string = "".to_string();
        let col = 0;
        let keybindings = Keybindings::new(&config_file["keybindings"]);
        Self {
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
            terminal,
            opener,
            keybindings,
        }
    }

    fn event_esc(&mut self) {
        self.input_string.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
        self.mode = Mode::Normal;
        self.col = 0;
    }

    fn event_up(&mut self) {
        if let Mode::Normal = self.mode {
            if self.file_index > 0 {
                self.file_index -= 1;
            }
            self.path_content.select_prev();
            self.window.scroll_up_one(self.file_index);
        }
    }

    fn event_down(&mut self) {
        if let Mode::Normal = self.mode {
            if self.file_index < self.path_content.files.len() - WINDOW_MARGIN_TOP {
                self.file_index += 1;
            }
            self.path_content.select_next();
            self.window.scroll_down_one(self.file_index);
        }
    }

    fn event_left(&mut self) {
        match self.mode {
            Mode::Normal => match self.path_content.path.parent() {
                Some(parent) => {
                    self.path_content =
                        PathContent::new(path::PathBuf::from(parent), self.args.hidden);
                    self.window.reset(self.path_content.files.len());
                    self.file_index = 0;
                    self.col = 0;
                }
                None => (),
            },
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search => {
                if self.col > 0 {
                    self.col -= 1
                }
            }
            _ => (),
        }
    }

    fn event_right(&mut self) {
        match self.mode {
            Mode::Normal => {
                if self.path_content.files[self.path_content.selected].is_dir {
                    self.path_content = PathContent::new(
                        self.path_content.files[self.path_content.selected]
                            .path
                            .clone(),
                        self.args.hidden,
                    );
                    self.window.reset(self.path_content.files.len());
                    self.file_index = 0;
                    self.col = 0;
                }
            }
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search => {
                if self.col < self.input_string.len() {
                    self.col += 1
                }
            }
            _ => (),
        }
    }

    fn event_backspace(&mut self) {
        match self.mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => {
                if self.col > 0 && !self.input_string.is_empty() {
                    self.input_string.remove(self.col - 1);
                    self.col -= 1;
                }
            }
            Mode::Normal => (),
            _ => (),
        }
    }

    fn event_char(&mut self, c: char) {
        match self.mode {
            Mode::Newfile
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Rename
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => self.event_text_insertion(c),
            Mode::Normal => {
                if c == self.keybindings.toggle_hidden {
                    self.event_toggle_hidden()
                } else if c == self.keybindings.copy_paste {
                    self.event_copy_paste()
                } else if c == self.keybindings.cut_paste {
                    self.event_cut_paste()
                } else if c == self.keybindings.newdir {
                    self.mode = Mode::Newdir
                } else if c == self.keybindings.newfile {
                    self.mode = Mode::Newfile
                } else if c == self.keybindings.chmod {
                    self.event_chmod()
                } else if c == self.keybindings.exec {
                    self.mode = Mode::Exec
                } else if c == self.keybindings.goto {
                    self.mode = Mode::Goto
                } else if c == self.keybindings.rename {
                    self.event_rename()
                } else if c == self.keybindings.clear_flags {
                    self.flagged.clear()
                } else if c == self.keybindings.toggle_flag {
                    self.event_toggle_flag()
                } else if c == self.keybindings.shell {
                    self.event_shell()
                } else if c == self.keybindings.delete {
                    self.event_delete()
                } else if c == self.keybindings.open_file {
                    self.event_open_file()
                } else if c == self.keybindings.help {
                    self.mode = Mode::Help
                } else if c == self.keybindings.search {
                    self.mode = Mode::Search
                } else if c == self.keybindings.quit {
                    std::process::exit(0)
                }
            }
            Mode::Help => {
                if c == self.keybindings.help {
                    self.mode = Mode::Normal
                } else if c == self.keybindings.quit {
                    std::process::exit(0);
                }
            }
        }
    }

    fn event_text_insertion(&mut self, c: char) {
        self.input_string.insert(self.col, c);
        self.col += 1;
    }

    fn event_toggle_flag(&mut self) {
        if self
            .flagged
            .contains(&self.path_content.files[self.file_index].path)
        {
            self.flagged
                .remove(&self.path_content.files[self.file_index].path);
        } else {
            self.flagged
                .insert(self.path_content.files[self.file_index].path.clone());
        }
        if self.file_index < self.path_content.files.len() - WINDOW_MARGIN_TOP {
            self.file_index += 1;
        }
        self.path_content.select_next();
        self.window.scroll_down_one(self.file_index);
    }

    fn event_toggle_hidden(&mut self) {
        self.args.hidden = !self.args.hidden;
        self.path_content.show_hidden = !self.path_content.show_hidden;
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len())
    }

    fn event_copy_paste(&mut self) {
        self.flagged.iter().for_each(|oldpath| {
            let newpath = self
                .path_content
                .path
                .clone()
                .join(oldpath.as_path().file_name().unwrap());
            fs::copy(oldpath, newpath).unwrap_or(0);
        });
        self.flagged.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    fn event_open_file(&mut self) {
        execute_in_child(
            &self.opener,
            &vec![self.path_content.files[self.path_content.selected]
                .path
                .to_str()
                .unwrap()],
        );
    }

    fn event_cut_paste(&mut self) {
        self.flagged.iter().for_each(|oldpath| {
            let newpath = self
                .path_content
                .path
                .clone()
                .join(oldpath.as_path().file_name().unwrap());
            fs::rename(oldpath, newpath).unwrap_or(());
        });
        self.flagged.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    fn event_rename(&mut self) {
        self.mode = Mode::Rename;
        let oldname = self.path_content.files[self.path_content.selected]
            .filename
            .clone();
        self.oldpath = self.path_content.path.to_path_buf();
        self.oldpath.push(oldname);
    }

    fn event_chmod(&mut self) {
        self.mode = Mode::Chmod;
        if self.flagged.is_empty() {
            self.flagged.insert(
                self.path_content.files[self.path_content.selected]
                    .path
                    .clone(),
            );
        }
    }

    fn event_shell(&mut self) {
        execute_in_child(
            &self.terminal,
            &vec!["-d", self.path_content.path.to_str().unwrap()],
        );
    }

    fn event_delete(&mut self) {
        self.flagged.iter().for_each(|pathbuf| {
            if pathbuf.is_dir() {
                fs::remove_dir_all(pathbuf).unwrap_or(());
            } else {
                fs::remove_file(pathbuf).unwrap_or(());
            }
        });
        self.flagged.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    fn event_home(&mut self) {
        if let Mode::Normal = self.mode {
            self.path_content.select_index(0);
            self.file_index = 0;
            self.window.scroll_to(0);
        }
    }

    fn event_end(&mut self) {
        if let Mode::Normal = self.mode {
            let last_index = self.path_content.files.len() - 1;
            self.path_content.select_index(last_index);
            self.file_index = last_index;
            self.window.scroll_to(last_index);
        }
    }

    fn event_page_down(&mut self) {
        if let Mode::Normal = self.mode {
            let down_index = min(self.path_content.files.len() - 1, self.file_index + 10);
            self.path_content.select_index(down_index);
            self.file_index = down_index;
            self.window.scroll_to(down_index);
        }
    }

    fn event_page_up(&mut self) {
        if let Mode::Normal = self.mode {
            let up_index = if self.file_index > 10 {
                self.file_index - 10
            } else {
                0
            };
            self.path_content.select_index(up_index);
            self.file_index = up_index;
            self.window.scroll_to(up_index);
        }
    }

    fn event_enter(&mut self) {
        match self.mode {
            Mode::Rename => self.exec_rename(),
            Mode::Newfile => self.exec_newfile(),
            Mode::Newdir => self.exec_newdir(),
            Mode::Chmod => self.exec_chmod(),
            Mode::Exec => self.exec_exec(),
            Mode::Search => self.exec_search(),
            Mode::Goto => self.exec_goto(),
            _ => (),
        }

        self.col = 0;
        self.mode = Mode::Normal;
    }

    fn refresh_view(&mut self) {
        self.input_string.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    fn exec_rename(&mut self) {
        fs::rename(
            self.oldpath.clone(),
            self.path_content
                .path
                .to_path_buf()
                .join(&self.input_string),
        )
        .unwrap_or(());
        self.refresh_view()
    }

    fn exec_newfile(&mut self) {
        if fs::File::create(self.path_content.path.join(self.input_string.clone())).is_ok() {}
        self.refresh_view()
    }

    fn exec_newdir(&mut self) {
        fs::create_dir(self.path_content.path.join(self.input_string.clone())).unwrap_or(());
        self.refresh_view()
    }

    fn exec_chmod(&mut self) {
        let permissions: u32 = u32::from_str_radix(&self.input_string, 8).unwrap_or(0_u32);
        if permissions <= MAX_PERMISSIONS {
            for path in self.flagged.iter() {
                Self::set_permissions(path.clone(), permissions).unwrap_or(())
            }
            self.flagged.clear()
        }
        self.input_string.clear();
        self.refresh_view()
    }

    fn set_permissions(path: path::PathBuf, permissions: u32) -> Result<(), std::io::Error> {
        fs::set_permissions(path, fs::Permissions::from_mode(permissions))
    }

    fn exec_exec(&mut self) {
        let exec_command = self.input_string.clone();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        args.push(
            self.path_content.files[self.path_content.selected]
                .path
                .to_str()
                .unwrap(),
        );
        self.input_string.clear();
        execute_in_child(command, &args);
    }

    fn event_left_click(&mut self, row: u16) {
        if let Mode::Normal = self.mode {
            self.file_index = (row - 1).into();
            self.path_content.select_index(self.file_index);
            self.window.scroll_to(self.file_index)
        }
    }
    fn event_right_click(&mut self, row: u16) {
        if let Mode::Normal = self.mode {
            self.file_index = (row - 1).into();
            self.path_content.select_index(self.file_index);
            self.window.scroll_to(self.file_index)
        }
        if self.path_content.files[self.file_index].is_dir {
            self.event_right()
        } else {
            self.event_open_file()
        }
    }

    fn exec_search(&mut self) {
        let searched_term = self.input_string.clone();
        let mut next_index = self.file_index;
        for (index, file) in self.path_content.files.iter().enumerate().skip(next_index) {
            if file.filename.contains(&searched_term) {
                next_index = index;
                break;
            };
        }
        self.input_string.clear();
        self.path_content.select_index(next_index);
        self.file_index = next_index;
        self.window.scroll_to(self.file_index);
    }

    fn exec_goto(&mut self) {
        let target = self.input_string.clone();
        self.input_string.clear();
        match std::fs::canonicalize(path::Path::new(&target)) {
            Ok(path) => {
                self.path_content = PathContent::new(path, self.args.hidden);
                self.window.reset(self.path_content.files.len());
            }
            Err(_) => (),
        }
    }

    fn set_height(&mut self, height: usize) {
        self.window.height = height;
        self.height = height;
    }
}

struct Display {
    term: Term,
}

impl Display {
    fn new(term: Term) -> Self {
        Self { term }
    }
    fn first_line(&mut self, status: &Status) {
        let first_row: String = match status.mode {
            Mode::Normal => {
                format!(
                    "h: {}, s: {} wt: {} wb: {}  m: {:?} - c: {:?} - {}",
                    status.height,
                    status.path_content.files.len(),
                    status.window.top,
                    status.window.bottom,
                    status.mode,
                    status.args,
                    status.path_content.path.to_str().unwrap()
                )
            }
            _ => {
                format!("{:?} {}", status.mode.clone(), status.input_string.clone())
            }
        };
        let _ = self.term.print(0, 0, &first_row);
    }

    fn files(&mut self, status: &Status) {
        let strings = status.path_content.strings();
        for (i, string) in strings
            .iter()
            .enumerate()
            .take(min(strings.len(), status.window.bottom))
            .skip(status.window.top)
        {
            let row = i + WINDOW_MARGIN_TOP - status.window.top;
            let mut attr = fileinfo_attr(&status.path_content.files[i], &status.colors);
            if status.flagged.contains(&status.path_content.files[i].path) {
                attr.effect |= Effect::UNDERLINE;
            }
            let _ = self.term.print_with_attr(row, 0, string, attr);
        }
    }

    fn help_or_cursor(&mut self, status: &Status) {
        match status.mode {
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
                let _ = self.term.set_cursor(0, status.col + EDIT_BOX_OFFSET);
            }
        }
    }
}

fn read_args() -> Config {
    let args = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        help();
        process::exit(1);
    });
    if args.help {
        help();
        process::exit(0);
    }
    args
}

fn main() {
    let args = read_args();
    let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    let _ = term.enable_mouse_support();
    let (_, height) = term.term_size().unwrap();
    let mut status = Status::new(args, height);
    let mut display = Display::new(term);
    while let Ok(ev) = display.term.poll_event() {
        let _ = display.term.clear();
        let (_width, height) = display.term.term_size().unwrap();
        status.set_height(height);
        match ev {
            Event::Key(Key::ESC) => status.event_esc(),
            Event::Key(Key::Up) => status.event_up(),
            Event::Key(Key::Down) => status.event_down(),
            Event::Key(Key::Left) => status.event_left(),
            Event::Key(Key::Right) => status.event_right(),
            Event::Key(Key::Backspace) => status.event_backspace(),
            Event::Key(Key::Char(c)) => status.event_char(c),
            Event::Key(Key::Home) => status.event_home(),
            Event::Key(Key::End) => status.event_end(),
            Event::Key(Key::PageDown) => status.event_page_down(),
            Event::Key(Key::PageUp) => status.event_page_up(),
            Event::Key(Key::Enter) => status.event_enter(),
            Event::Key(Key::WheelUp(_, _, _)) => status.event_up(),
            Event::Key(Key::WheelDown(_, _, _)) => status.event_down(),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => status.event_left_click(row),
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                status.event_right_click(row)
            }
            _ => {}
        }

        display.first_line(&status);
        display.files(&status);
        display.help_or_cursor(&status);

        let _ = display.term.present();
    }
}
