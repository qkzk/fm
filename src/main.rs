use std::cmp::min;
use std::collections::HashSet;
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
    // Chmod,
    Newfile,
    // Newdir,
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
    let mut flagged = HashSet::new();
    let mut new_filename = "".to_string();

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
                    flagged.clear();
                }
                None => (),
            },
            Event::Key(Key::Right) => {
                if path_content.files[path_content.selected].is_dir {
                    path_content = PathContent::new(
                        path_content.files[path_content.selected].path.clone(),
                        config.hidden,
                    );
                    col = Col::default();
                    window.reset(path_content.files.len());
                    flagged.clear();
                }
            }
            Event::Key(Key::Ctrl('a')) => {
                config.hidden = !config.hidden;
                path_content.show_hidden = !path_content.show_hidden;
                path_content.reset_files();
            }
            Event::Key(Key::Ctrl('r')) => {
                if let Mode::Normal = mode {
                    mode = Mode::Rename;
                    let oldname = path_content.files[path_content.selected].filename.clone();
                    oldpath = path_content.path.to_path_buf();
                    oldpath.push(oldname);
                    path_content.files[path_content.selected].filename = "".to_string();
                }
            }
            Event::Key(Key::Char(c)) => match mode {
                Mode::Rename => {
                    path_content.files[path_content.selected].filename.push(c);
                }
                Mode::Newfile => {
                    new_filename.push(c);
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
                    'n' => {
                        mode = Mode::Newfile;
                    }
                    'u' => {
                        flagged.clear();
                    }
                    'x' => {
                        flagged.iter().for_each(|pathbuf| {
                            fs::remove_file(pathbuf).expect("Couldn't remove file");
                        });
                        flagged.clear();
                        path_content = PathContent::new(path_content.path, config.hidden);
                        window.reset(path_content.files.len());
                    }
                    _ => (),
                },
            },
            Event::Key(Key::Enter) => {
                if let Mode::Rename = mode {
                    let mut newpath = path_content.path.to_path_buf();
                    newpath.push(path_content.files[path_content.selected].filename.clone());
                    fs::rename(oldpath.clone(), newpath).expect("Couldn't rename the file");
                } else if let Mode::Newfile = mode {
                    fs::File::create(path_content.path.join(new_filename.clone()))
                        .expect("Couldn't create file");
                    new_filename.clear();
                    path_content = PathContent::new(path_content.path, config.hidden);
                    window.reset(path_content.files.len());
                }
                mode = Mode::Normal;
            }
            _ => {}
        }
        path_text = path_content.path.to_str().unwrap();
        let normal_first_row = format!(
            "h: {}, s: {} wt: {} wb: {}  m: {:?} - c: {:?} - {}",
            height,
            path_content.files.len(),
            window.top,
            window.bottom,
            mode,
            config,
            path_text
        );
        let new_file_first_row = format!("New file: {}", new_filename);
        let _ = term.print(
            0,
            0,
            match mode {
                Mode::Newfile => &new_file_first_row,
                _ => &normal_first_row,
            },
        );
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
            if path_content.files[i].is_selected {
                let _ = term.set_cursor(row, col.col);
            }
        }

        let _ = term.present();
    }
}
