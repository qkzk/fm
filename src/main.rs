use std::cmp::min;
use std::fs;
use std::{env, path, process};

use tuikit::attr::*;
use tuikit::event::{Event, Key};
use tuikit::term::{Term, TermHeight};

use fm::config::Config;
use fm::fileinfo::{FileInfo, PathContent};

pub mod fileinfo;

// fn main2() -> Result<(), io::Error> {
//     let stdout = io::stdout();
//     let backend = CrosstermBackend::new(stdout);
//     let mut terminal = Terminal::new(backend)?;
//     Ok(())
// }

// fn main1() {
//     let path = expand(path::Path::new(".")).unwrap();
//     let parent = path.parent();
//     println!("{:?}, {:?}", path, parent);
// }

const PERM_COL: usize = 0;
const OWNE_COL: usize = 16;
const NAME_COL: usize = 44;
const WINDOW_PADDING: usize = 4;
const WINDOW_MARGIN_TOP: usize = 1;

struct Col {
    col: usize,
}

impl Col {
    pub fn new() -> Self {
        Self { col: NAME_COL }
    }

    pub fn prev(&mut self) {
        match self.col {
            PERM_COL => self.col = NAME_COL,
            OWNE_COL => self.col = PERM_COL,
            NAME_COL => self.col = OWNE_COL,
            _ => (),
        }
    }

    pub fn next(&mut self) {
        match self.col {
            PERM_COL => self.col = OWNE_COL,
            OWNE_COL => self.col = NAME_COL,
            NAME_COL => self.col = PERM_COL,
            _ => (),
        }
    }
}

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
            bottom: min(len, height),
            len,
            height,
        }
    }

    fn move_down(&mut self) {
        self.top += 1;
        self.bottom += 1
    }

    fn move_up(&mut self) {
        self.top -= 1;
        self.bottom -= 1;
    }

    fn scroll_up_to(&mut self, row: usize) {
        if row < self.top + WINDOW_PADDING && self.top > 0 {
            self.move_up();
        }
    }
    fn scroll_down_to(&mut self, row: usize) {
        if self.len < self.height {
            return;
        }
        if row > self.bottom - WINDOW_PADDING && self.bottom < self.len - WINDOW_MARGIN_TOP {
            self.move_down();
        }
    }

    fn reset(&mut self, len: usize) {
        self.len = len;
        self.top = 0;
        self.bottom = min(len, self.height);
    }
}

impl Default for Col {
    fn default() -> Self {
        Self::new()
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

#[derive(Debug)]
enum Mode {
    Normal,
    Rename,
    Chmod,
    Newfile,
    Newdir,
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
    let mut path_text: &str;
    let mut file_index = 0;
    let mut col = Col::default();
    let mut window = FilesWindow::new(path_content.files.len(), height);
    let mut oldpath: path::PathBuf = path::PathBuf::new();

    while let Ok(ev) = term.poll_event() {
        let _ = term.clear();
        let (_width, height) = term.term_size().unwrap();
        window.height = height;
        match ev {
            Event::Key(Key::ESC | Key::Ctrl('q')) => break,
            Event::Key(Key::Up) => {
                if file_index > 0 {
                    file_index -= 1;
                }
                path_content.select_prev();
                window.scroll_up_to(file_index);
            }
            Event::Key(Key::Down) => {
                if file_index < path_content.files.len() - WINDOW_MARGIN_TOP {
                    file_index += 1;
                }
                path_content.select_next();
                window.scroll_down_to(file_index);
            }
            Event::Key(Key::CtrlLeft) => col.prev(),
            Event::Key(Key::CtrlRight) => col.next(),
            Event::Key(Key::Left) => match path_content.path.parent() {
                Some(parent) => {
                    path_content = PathContent::new(path::PathBuf::from(parent), config.hidden);
                    col = Col::default();
                    window.reset(path_content.files.len());
                }
                None => (),
            },
            Event::Key(Key::Right) => {
                if path_content.files[path_content.selected].is_dir {
                    let mut pb = path_content.path.to_path_buf();
                    pb.push(path_content.files[path_content.selected].filename.clone());
                    path_content = PathContent::new(pb, config.hidden);
                    col = Col::default();
                    window.reset(path_content.files.len());
                }
            }
            Event::Key(Key::Ctrl('a')) => {
                config.hidden = !config.hidden;
                path_content.show_hidden = !path_content.show_hidden;
                path_content.reset_files();
            }
            Event::Key(Key::Ctrl('r')) => match mode {
                Mode::Normal => {
                    mode = Mode::Rename;
                    let oldname = path_content.files[path_content.selected].filename.clone();
                    oldpath = path_content.path.to_path_buf();
                    oldpath.push(oldname);
                    path_content.files[path_content.selected].filename = "".to_string();
                }
                _ => (),
            },
            Event::Key(Key::Char(c)) => match mode {
                Mode::Rename => {
                    path_content.files[path_content.selected].filename.push(c);
                }
                _ => (),
            },
            Event::Key(Key::Enter) => {
                match mode {
                    Mode::Rename => {
                        let mut newpath = path_content.path.to_path_buf();
                        newpath.push(path_content.files[path_content.selected].filename.clone());
                        fs::rename(oldpath.clone(), newpath).expect("Couldn't rename the file");
                    }
                    _ => (),
                }
                mode = Mode::Normal;
            }
            _ => {}
        }
        path_text = path_content.path.to_str().unwrap();
        let _ = term.print(
            0,
            0,
            &format!(
                "h: {}, s: {} wt: {} wb: {}  m: {:?} - c: {:?} - {}",
                height,
                path_content.files.len(),
                window.top,
                window.bottom,
                mode,
                config,
                path_text
            ),
        );
        let strings = path_content.strings();
        for (i, string) in strings
            .iter()
            .enumerate()
            .take(min(strings.len(), window.bottom))
            .skip(window.top)
        {
            let row = i + WINDOW_MARGIN_TOP - window.top;
            let _ = term.print_with_attr(row, 0, string, fileinfo_attr(&path_content.files[i]));
            if path_content.files[i].is_selected {
                let _ = term.set_cursor(row, col.col);
            }
        }

        let _ = term.present();
    }
}
