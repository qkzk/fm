use std::io::{BufRead, Read};
use std::path::PathBuf;

use content_inspector::{inspect, ContentType};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tuikit::attr::{Attr, Color};
use tuikit::term::Term;

use crate::fileinfo::PathContent;

pub enum Preview {
    SyntaxedPreview(SyntaxedContent),
    TextPreview(TextContent),
    Binary(BinaryContent),
    Empty,
}

impl Preview {
    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn new(path_content: &PathContent) -> Self {
        let ps = SyntaxSet::load_defaults_nonewlines();
        match path_content.selected_file() {
            Some(file) => {
                let mut f = std::fs::File::open(file.path.clone()).unwrap();
                let mut buffer = vec![0; 1024];
                f.read_exact(&mut buffer).unwrap();
                if inspect(&buffer) == ContentType::BINARY {
                    Self::Binary(BinaryContent::new(path_content.to_owned()))
                } else {
                    if let Some(syntaxset) = ps.find_syntax_by_extension(&file.extension) {
                        Self::SyntaxedPreview(SyntaxedContent::new(
                            ps.clone(),
                            path_content,
                            syntaxset,
                        ))
                    } else {
                        Self::TextPreview(TextContent::from_file(path_content))
                    }
                }
            }
            None => Self::Empty,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::SyntaxedPreview(syntaxed) => syntaxed.len(),
            Self::TextPreview(text) => text.len(),
            Self::Empty => 0,
            Self::Binary(binary) => binary.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone)]
pub struct TextContent {
    pub content: Box<Vec<String>>,
    length: usize,
}

impl Default for TextContent {
    fn default() -> Self {
        Self {
            content: Box::new(vec![]),
            length: 0,
        }
    }
}

impl TextContent {
    pub fn from_file(path_content: &PathContent) -> Self {
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
        Self {
            length: content.len(),
            content,
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

pub struct SyntaxedContent {
    pub highlighted_content: Box<Vec<Vec<SyntaxedString>>>,
    length: usize,
}

impl Default for SyntaxedContent {
    fn default() -> Self {
        Self {
            highlighted_content: Box::new(vec![vec![]]),
            length: 0,
        }
    }
}

impl SyntaxedContent {
    pub fn new(ps: SyntaxSet, path_content: &PathContent, syntaxset: &SyntaxReference) -> Self {
        let file = path_content.selected_file().unwrap();
        let reader = std::io::BufReader::new(std::fs::File::open(file.path.clone()).unwrap());
        let content: Box<Vec<String>> = Box::new(
            reader
                .lines()
                .map(|line| line.unwrap_or_else(|_| "".to_owned()))
                .collect(),
        );
        let ts = ThemeSet::load_defaults();
        let mut highlighted_content = Box::new(vec![]);
        let syntax = syntaxset.to_owned();
        let mut highlight_line = HighlightLines::new(&syntax, &ts.themes["Solarized (dark)"]);

        for line in content.iter() {
            let mut col = 0;
            let mut v_line = vec![];
            if let Ok(v) = highlight_line.highlight_line(line, &ps) {
                for (style, token) in v.iter() {
                    v_line.push(SyntaxedString::from_syntect(col, token.to_string(), *style));
                    col += token.len();
                }
            }
            highlighted_content.push(v_line)
        }

        Self {
            length: highlighted_content.len(),
            highlighted_content,
        }
    }

    pub fn reset(&mut self) {
        self.highlighted_content = Box::new(vec![vec![]])
    }

    fn len(&self) -> usize {
        self.length
    }
}

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

    pub fn print(&self, term: &Term, row: usize, offset: usize) {
        let _ = term.print_with_attr(row, self.col + offset + 2, &self.content, self.attr);
    }
}

pub struct BinaryContent {
    pub path: PathBuf,
    length: u64,
}

impl BinaryContent {
    fn new(path_content: PathContent) -> Self {
        let file = path_content.selected_file().unwrap();

        Self {
            path: file.path.clone(),
            length: file.size,
        }
    }

    fn len(&self) -> usize {
        self.length as usize
    }
}
