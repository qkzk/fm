use std::io::BufRead;
use std::iter::Skip;

use syntect::easy::HighlightFile;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use tuikit::attr::*;
use tuikit::term::Term;

use crate::fileinfo::PathContent;

pub struct Preview {
    pub content: Box<Vec<String>>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            content: Box::new(vec![]),
        }
    }
}

impl Preview {
    pub fn fill_preview_lines(&mut self, path_content: &PathContent) {
        self.content = match path_content.selected_file() {
            Some(file) => {
                let reader =
                    std::io::BufReader::new(std::fs::File::open(file.path.clone()).unwrap());
                Box::new(
                    reader
                        .lines()
                        .map(|line| line.unwrap_or_else(|_| "".to_owned()))
                        .collect(),
                )
            }
            None => Box::new(vec![]),
        };
    }

    pub fn empty_preview_lines(&mut self) {
        self.content = Box::new(vec![])
    }

    // pub fn bla(&self, path_content: &PathContent) -> HighlightFile {
    //     HighlightFile::new(
    //         path_content.selected_file().unwrap().path.clone(),
    //         &ss,
    //          SyntaxSet::load_defaults_nonewlines();
    //         &ThemeSet::load_defaults().themes["InspiredGitHub"].clone(),
    //     )
    //     .unwrap()
    // }
    // TODO: ref : https://github.com/trishume/syntect/blob/master/examples/latex-demo.rs
}

// pub struct Highlighter {
//     ps: SyntaxSet,
//     ts: ThemeSet,
// }
//
// impl Highlighter {
//     pub fn preview(&mut self, code: Skip<Vec<String>>, term: &Term) {
//         let syntax = self.ps.find_syntax_by_extension("yml").unwrap();
//         let mut h = HighlightLines::new(syntax, &self.ts.themes["base16-ocean.light"]);
//
//         for (row, line) in code.iter().enumerate() {
//             for (col, (style, s)) in h.highlight_line(line, &self.ps).iter().enumerate() {
//                 let fg = style.foreground;
//                 let attr = Attr {
//                     fg: Color::Rgb(fg.r, fg.g, fg.b),
//                     ..Default::default()
//                 };
//                 let _ = term.print_with_attr(row + 2, col + 5, s, attr);
//             }
//         }
//     }
// }
//
// impl Highlighter {
//     pub fn new() -> Highlighter {
//         Highlighter {
//             ps: SyntaxSet::load_defaults_newlines(),
//             ts: ThemeSet::load_defaults(),
//         }
//     }
// }
//
// impl Default for Highlighter {
//     fn default() -> Self {
//         Highlighter::new()
//     }
// }
