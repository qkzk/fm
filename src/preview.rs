use std::fmt::Write as _;
use std::io::{BufRead, Read};
use std::path::PathBuf;

use content_inspector::{inspect, ContentType};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tuikit::attr::{Attr, Color};
use tuikit::term::Term;

use crate::fileinfo::PathContent;

#[derive(Clone)]
pub enum Preview {
    Syntaxed(SyntaxedContent),
    Text(TextContent),
    Binary(BinaryContent),
    Empty,
}

impl Preview {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn new(path_content: &PathContent) -> Self {
        let ps = SyntaxSet::load_defaults_nonewlines();
        match path_content.selected_file() {
            Some(file_info) => {
                let mut file = std::fs::File::open(file_info.path.clone()).unwrap();
                let mut buffer = vec![0; Self::CONTENT_INSPECTOR_MIN_SIZE];
                if let Some(syntaxset) = ps.find_syntax_by_extension(&file_info.extension) {
                    Self::Syntaxed(SyntaxedContent::new(ps.clone(), path_content, syntaxset))
                } else if file_info.size >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
                    && file.read_exact(&mut buffer).is_ok()
                    && inspect(&buffer) == ContentType::BINARY
                {
                    Self::Binary(BinaryContent::new(path_content.to_owned()))
                } else {
                    Self::Text(TextContent::from_file(path_content))
                }
            }
            None => Self::Empty,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Syntaxed(syntaxed) => syntaxed.len(),
            Self::Text(text) => text.len(),
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

#[derive(Clone)]
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

#[derive(Clone)]
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

#[derive(Clone)]
pub struct BinaryContent {
    pub path: PathBuf,
    length: u64,
    pub content: Box<Vec<Line>>,
}

impl BinaryContent {
    const LINE_WIDTH: usize = 16;

    fn new(path_content: PathContent) -> Self {
        let file = path_content.selected_file().unwrap();
        let mut reader = std::io::BufReader::new(std::fs::File::open(file.path.clone()).unwrap());
        let mut buffer = [0; Self::LINE_WIDTH];
        let mut content: Box<Vec<Line>> = Box::new(vec![]);
        while let Ok(n) = reader.read(&mut buffer[..]) {
            if n != Self::LINE_WIDTH {
                content.push(Line::new((&buffer[0..n]).into()));
                break;
            } else {
                content.push(Line::new(buffer.into()));
            }
        }

        Self {
            path: file.path.clone(),
            length: file.size / Self::LINE_WIDTH as u64,
            content,
        }
    }

    /// WATCHOUT !
    /// Doesn't return the size of the file, like similar methods in other variants.
    /// It returns the number of **lines**.
    /// It's the size of the file divided by `BinaryContent::LINE_WIDTH` which is 16.
    pub fn len(&self) -> usize {
        self.length as usize
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

/// Holds a `Vec` of "bytes" (`u8`).
/// It's mostly used to implement a `print` method.
#[derive(Clone)]
pub struct Line {
    line: Vec<u8>,
}

impl Line {
    fn new(line: Vec<u8>) -> Self {
        Self { line }
    }

    fn format(&self) -> String {
        let mut s = "".to_owned();
        for (i, byte) in self.line.iter().enumerate() {
            let _ = write!(s, "{:02x}", byte);
            if i % 2 == 1 {
                s.push(' ');
            }
        }
        s
    }

    /// Print line of pair of bytes in hexadecimal, 16 bytes long.
    /// It imitates the output of hexdump.
    pub fn print(&self, term: &Term, row: usize, offset: usize) {
        let _ = term.print(row, offset + 2, &self.format());
    }
}
