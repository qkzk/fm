use std::cmp::{max, min};
use std::path;
use tuikit::attr::*;
use tuikit::event::{Event, Key};
use tuikit::term::{Term, TermHeight};

use fm::fileinfo::{expand, PathContent};

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

impl Default for Col {
    fn default() -> Self {
        Self::new()
    }
}

fn main() {
    let path = expand(path::Path::new(".")).unwrap();
    let mut path_content = PathContent::new(path);

    let mut text = path_content.path.to_str().unwrap();
    let term: Term<()> = Term::with_height(TermHeight::Percent(30)).unwrap();
    let mut row = 1;
    let mut col = Col::default();

    let _ = term.print(0, 0, text);
    let _ = term.present();

    while let Ok(ev) = term.poll_event() {
        text = path_content.path.to_str().unwrap();
        let _ = term.clear();
        let _ = term.print(0, 0, text);

        // let filechild = path_content.files[path_content.selected].filename.clone();
        // let pathbufchild = path_content.path.to_path_buf().join(&filechild);
        // let pathchild = pathbufchild.as_path();

        let (width, height) = term.term_size().unwrap();
        match ev {
            Event::Key(Key::ESC) | Event::Key(Key::Char('q')) => break,
            Event::Key(Key::Up) => {
                row = max(row - 1, 1);
                path_content.select_prev();
            }
            Event::Key(Key::Down) => {
                row = min(row + 1, min(height - 1, path_content.files.len()));
                path_content.select_next();
            }
            Event::Key(Key::CtrlLeft) => col.prev(),
            Event::Key(Key::CtrlRight) => col.next(),
            Event::Key(Key::Left) => match path_content.path.parent() {
                Some(parent) => {
                    path_content = PathContent::new(path::PathBuf::from(parent));
                    row = 1;
                    col = Col::default();
                }
                None => (),
            },
            Event::Key(Key::Right) => {
                if path_content.files[path_content.selected].is_dir {
                    let mut pb = path_content.path.to_path_buf();
                    pb.push(path_content.files[path_content.selected].filename.clone());
                    path_content = PathContent::new(pb);
                    row = 1;
                    col = Col::default();
                }
            }
            _ => {}
        }

        let attr_unselected = Attr {
            fg: Color::LIGHT_CYAN,
            ..Attr::default()
        };
        let attr_selected = Attr {
            fg: Color::RED,
            ..Attr::default()
        };
        for (i, string) in path_content.strings().into_iter().enumerate() {
            let _ = term.print_with_attr(
                i + 1,
                0,
                &string,
                if path_content.files[i].is_selected {
                    attr_selected
                } else {
                    attr_unselected
                },
            );
        }
        let _ = term.set_cursor(row, col.col);
        let _ = term.present();
    }
}
