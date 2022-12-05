use std::cmp::min;
use std::fmt::Write as _;
use std::io::{BufRead, Read};
use std::iter::{Enumerate, Skip, Take};
use std::panic;
use std::path::PathBuf;
use std::slice::Iter;

use content_inspector::{inspect, ContentType};
use pdf_extract;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tuikit::attr::{Attr, Color};
use zip::ZipArchive;

use crate::fileinfo::PathContent;
use crate::fm_error::{FmError, FmResult};

#[derive(Clone)]
pub enum Preview {
    Syntaxed(SyntaxedContent),
    Text(TextContent),
    Binary(BinaryContent),
    Pdf(PdfContent),
    Compressed(CompressedContent),
    Image(ExifContent),
    Media(MediainfoContent),
    Empty,
}

fn is_ext_image(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "tiff" | "heif")
}

fn is_ext_media(ext: &str) -> bool {
    matches!(
        ext,
        "mkv"
            | "ogg"
            | "ogm"
            | "riff"
            | "mpeg"
            | "mp2"
            | "mp3"
            | "mp4"
            | "wm"
            | "qt"
            | "ac3"
            | "dts"
            | "aac"
            | "mac"
            | "flac"
            | "avi"
    )
}

impl Preview {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    pub fn empty() -> Self {
        Self::Empty
    }

    pub fn new(path_content: &PathContent) -> FmResult<Self> {
        match path_content.selected_file() {
            Some(file_info) => match file_info.extension.to_lowercase().as_str() {
                "zip" => Ok(Self::Compressed(CompressedContent::new(
                    file_info.path.clone(),
                )?)),
                "pdf" => Ok(Self::Pdf(PdfContent::new(file_info.path.clone()))),
                e if is_ext_image(e) => Ok(Self::Image(ExifContent::new(file_info.path.clone())?)),
                e if is_ext_media(e) => {
                    Ok(Self::Media(MediainfoContent::new(file_info.path.clone())?))
                }
                _ => {
                    let mut file = std::fs::File::open(file_info.path.clone())?;
                    let mut buffer = vec![0; Self::CONTENT_INSPECTOR_MIN_SIZE];
                    let ps = SyntaxSet::load_defaults_nonewlines();
                    if let Some(syntaxset) = ps.find_syntax_by_extension(&file_info.extension) {
                        Ok(Self::Syntaxed(SyntaxedContent::new(
                            ps.clone(),
                            path_content,
                            syntaxset,
                        )?))
                    } else if file_info.size >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
                        && file.read_exact(&mut buffer).is_ok()
                        && inspect(&buffer) == ContentType::BINARY
                    {
                        Ok(Self::Binary(BinaryContent::new(path_content.to_owned())?))
                    } else {
                        Ok(Self::Text(TextContent::from_file(path_content)?))
                    }
                }
            },
            None => Ok(Self::Empty),
        }
    }

    pub fn help(help: String) -> Self {
        Self::Text(TextContent::help(help))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Syntaxed(syntaxed) => syntaxed.len(),
            Self::Text(text) => text.len(),
            Self::Binary(binary) => binary.len(),
            Self::Pdf(pdf) => pdf.len(),
            Self::Compressed(zip) => zip.len(),
            Self::Image(img) => img.len(),
            Self::Media(media) => media.len(),
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
    fn help(help: String) -> Self {
        let content: Box<Vec<String>> = Box::new(help.split('\n').map(|s| s.to_owned()).collect());
        Self {
            length: content.len(),
            content,
        }
    }

    fn from_file(path_content: &PathContent) -> FmResult<Self> {
        let content = match path_content.selected_file() {
            Some(file) => {
                let reader = std::io::BufReader::new(std::fs::File::open(file.path.clone())?);
                Box::new(
                    reader
                        .lines()
                        .map(|line| line.unwrap_or_else(|_| "".to_owned()))
                        .collect(),
                )
            }
            None => Box::new(vec![]),
        };
        Ok(Self {
            length: content.len(),
            content,
        })
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
    pub content: Box<Vec<Vec<SyntaxedString>>>,
    length: usize,
}

impl Default for SyntaxedContent {
    fn default() -> Self {
        Self {
            content: Box::new(vec![vec![]]),
            length: 0,
        }
    }
}

impl SyntaxedContent {
    pub fn new(
        ps: SyntaxSet,
        path_content: &PathContent,
        syntaxset: &SyntaxReference,
    ) -> FmResult<Self> {
        let file = path_content
            .selected_file()
            .ok_or_else(|| FmError::new("Path shouldn't be empty"))?;
        let reader = std::io::BufReader::new(std::fs::File::open(file.path.clone())?);
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

        Ok(Self {
            length: highlighted_content.len(),
            content: highlighted_content,
        })
    }

    pub fn reset(&mut self) {
        self.content = Box::new(vec![vec![]])
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

    pub fn print(
        &self,
        canvas: &mut dyn tuikit::canvas::Canvas,
        row: usize,
        offset: usize,
    ) -> FmResult<()> {
        canvas.print_with_attr(row, self.col + offset + 2, &self.content, self.attr)?;
        Ok(())
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

    fn new(path_content: PathContent) -> FmResult<Self> {
        let file = path_content
            .selected_file()
            .ok_or_else(|| FmError::new("Path shouldn't be emtpy"))?;
        let mut reader = std::io::BufReader::new(std::fs::File::open(file.path.clone())?);
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

        Ok(Self {
            path: file.path.clone(),
            length: file.size / Self::LINE_WIDTH as u64,
            content,
        })
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
    pub fn print(&self, canvas: &mut dyn tuikit::canvas::Canvas, row: usize, offset: usize) {
        let _ = canvas.print(row, offset + 2, &self.format());
    }
}

#[derive(Clone)]
pub struct PdfContent {
    length: usize,
    pub content: Vec<String>,
}

impl PdfContent {
    fn new(path: PathBuf) -> Self {
        let result = catch_unwind_silent(|| {
            if let Ok(content_string) = pdf_extract::extract_text(path) {
                content_string
                    .split_whitespace()
                    .map(|s| s.to_owned())
                    .collect()
            } else {
                vec!["Coudln't parse the pdf".to_owned()]
            }
        });
        let content = result.unwrap_or_else(|_| vec!["Couldn't read the pdf".to_owned()]);

        Self {
            length: content.len(),
            content,
        }
    }

    fn len(&self) -> usize {
        self.length
    }
}

#[derive(Clone)]
pub struct CompressedContent {
    length: usize,
    pub content: Vec<String>,
}

impl CompressedContent {
    fn new(path: PathBuf) -> FmResult<Self> {
        let reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let zip = ZipArchive::new(reader)?;
        let mut content_str: Vec<&str> = zip.file_names().collect();
        content_str.sort();
        let content: Vec<String> = content_str.iter().map(|s| (*s).to_owned()).collect();

        Ok(Self {
            length: content.len(),
            content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }
}

#[derive(Clone)]
pub struct ExifContent {
    length: usize,
    pub content: Vec<String>,
}

impl ExifContent {
    fn new(path: PathBuf) -> FmResult<Self> {
        let mut bufreader = std::io::BufReader::new(std::fs::File::open(path)?);
        let exif = exif::Reader::new().read_from_container(&mut bufreader)?;
        let content: Vec<String> = exif
            .fields()
            .map(|f| Self::format_exif_field(f, &exif))
            .collect();
        Ok(Self {
            length: content.len(),
            content,
        })
    }

    fn format_exif_field(f: &exif::Field, exif: &exif::Exif) -> String {
        format!(
            "{} {} {}",
            f.tag,
            f.ifd_num,
            f.display_value().with_unit(exif)
        )
    }

    fn len(&self) -> usize {
        self.length
    }
}

#[derive(Clone)]
pub struct MediainfoContent {
    length: usize,
    pub content: Vec<String>,
}

impl MediainfoContent {
    fn new(path: PathBuf) -> FmResult<Self> {
        let content: Vec<String>;
        if let Ok(output) = std::process::Command::new("mediainfo").arg(path).output() {
            let s = String::from_utf8(output.stdout).unwrap_or_default();
            content = s.lines().map(|s| s.to_owned()).collect();
        } else {
            content = vec![];
        }
        Ok(Self {
            length: content.len(),
            content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }
}

pub trait Window<T> {
    fn window(
        &self,
        top: usize,
        bottom: usize,
        length: usize,
    ) -> Take<Skip<Enumerate<Iter<'_, T>>>>;
}

impl Window<Vec<SyntaxedString>> for SyntaxedContent {
    fn window(
        &self,
        top: usize,
        bottom: usize,
        length: usize,
    ) -> std::iter::Take<Skip<Enumerate<Iter<'_, Vec<SyntaxedString>>>>> {
        self.content
            .iter()
            .enumerate()
            .skip(top)
            .take(min(length, bottom + 1))
    }
}

macro_rules! impl_window {
    ($t:ident, $u:ident) => {
        impl Window<$u> for $t {
            fn window(
                &self,
                top: usize,
                bottom: usize,
                length: usize,
            ) -> Take<Skip<Enumerate<Iter<'_, $u>>>> {
                self.content
                    .iter()
                    .enumerate()
                    .skip(top)
                    .take(min(length, bottom + 1))
            }
        }
    };
}

impl_window!(TextContent, String);
impl_window!(BinaryContent, Line);
impl_window!(PdfContent, String);
impl_window!(CompressedContent, String);
impl_window!(ExifContent, String);
impl_window!(MediainfoContent, String);

fn catch_unwind_silent<F: FnOnce() -> R + panic::UnwindSafe, R>(f: F) -> std::thread::Result<R> {
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(f);
    panic::set_hook(prev_hook);
    result
}
