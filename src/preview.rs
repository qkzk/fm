use std::io::BufRead;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use tuikit::attr::{Attr, Color};
use tuikit::term::Term;

use crate::fileinfo::PathContent;

pub struct SyntaxedString {
    // row: usize,
    col: usize,
    content: String,
    attr: Attr,
}

impl SyntaxedString {
    pub fn from_syntect(col: usize, content: String, style: Style) -> Self {
        let fg = style.foreground;
        let attr = Attr {
            fg: Color::Rgb(fg.r, fg.g, fg.b),
            ..Default::default()
        };
        Self {
            // row,
            col,
            content,
            attr,
        }
    }

    pub fn print(&self, term: &Term, row: usize) {
        let _ = term.print_with_attr(row, self.col + 5, &self.content, self.attr);
    }
}

pub struct Preview {
    pub highlighted_content: Box<Vec<Vec<SyntaxedString>>>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            highlighted_content: Box::new(vec![vec![]]),
        }
    }
}

impl Preview {
    pub fn new(path_content: &PathContent) -> Self {
        let content = match path_content.selected_file() {
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
        let ps = SyntaxSet::load_defaults_nonewlines();
        let ts = ThemeSet::load_defaults();
        let mut highlighted_content = Box::new(vec![]);
        match path_content.selected_file() {
            Some(file) => {
                if let Some(syntaxset) = ps.find_syntax_by_extension(&file.extension) {
                    let syntax = syntaxset.to_owned();
                    let mut highlight_line =
                        HighlightLines::new(&syntax, &ts.themes["Solarized (dark)"]);

                    for line in content.iter() {
                        let mut col = 0;
                        let mut v_line = vec![];
                        if let Ok(v) = highlight_line.highlight_line(line, &ps) {
                            for (style, token) in v.iter() {
                                v_line.push(SyntaxedString::from_syntect(
                                    col,
                                    token.to_string(),
                                    *style,
                                ));
                                col += token.len();
                            }
                        }
                        highlighted_content.push(v_line)
                    }
                }
            }
            None => (),
        }

        Self {
            highlighted_content,
        }
    }

    pub fn reset(&mut self) {
        self.highlighted_content = Box::new(vec![vec![]])
    }
}
