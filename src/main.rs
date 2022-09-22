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

fn main1() {
    let path = expand(path::Path::new(".")).unwrap();
    let parent = path.parent();
    println!("{:?}, {:?}", path, parent);
}

fn main() {
    let path = expand(path::Path::new(".")).unwrap();
    let mut path_content = PathContent::new(&path);

    let text = path_content.path.to_str().unwrap();
    let term: Term<()> = Term::with_height(TermHeight::Percent(30)).unwrap();
    let mut row = 1;
    let mut col = 0;

    let _ = term.print(0, 0, text);
    let _ = term.present();

    while let Ok(ev) = term.poll_event() {
        let text = path_content.path.to_str().unwrap();
        let _ = term.clear();
        let _ = term.print(0, 0, text);

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
            // Event::Key(Key::Left) => col = max(col, 1) - 1,
            Event::Key(Key::Left) => match path_content.path.parent() {
                Some(parent) => path_content = PathContent::new(parent),
                None => (),
            },
            // Event::Key(Key::Right) => col = min(col + 1, width - 1),
            Event::Key(Key::Right) => {
                // let filechild = path_content.files[path_content.selected].filename.clone();
                // if path_content.files[path_content.selected].is_dir {
                //     path_content =
                //         PathContent::new(path_content.path.to_path_buf().join(filechild).as_path());

                // path_content.child();
                // let mut pb = path_content.path.to_path_buf();
                // pb.push(path_content.files[path_content.selected].filename.clone());
                // path_content = PathContent::new(pb.as_path());
                // path_content.path = pb.as_path();
                // TODO: pb does not live long enough
                // https://users.rust-lang.org/t/how-to-resolve-error-e0515-cannot-return-value-referencing-temporary-value-without-owned-value/43132
                // }
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
        let _ = term.set_cursor(row, col);
        let _ = term.present();
    }
}
