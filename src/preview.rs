use std::cmp::min;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read};
use std::iter::{Enumerate, Skip, Take};
use std::panic;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::slice::Iter;

use content_inspector::{inspect, ContentType};
use image::imageops::FilterType;
use image::{ImageBuffer, Rgb};
use pdf_extract;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tuikit::attr::{Attr, Color};
use users::UsersCache;

use crate::compress::list_files;
use crate::config::Colors;
use crate::fileinfo::{FileInfo, FileKind};
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::status::Status;
use crate::tree::{ColoredString, Tree};

/// Different kind of preview used to display some informaitons
/// About the file.
/// We check if it's an archive first, then a pdf file, an image, a media file
#[derive(Clone)]
pub enum Preview {
    Syntaxed(HLContent),
    Text(TextContent),
    Binary(BinaryContent),
    Pdf(PdfContent),
    Archive(ZipContent),
    Exif(ExifContent),
    Thumbnail(Pixels),
    Media(MediaContent),
    Directory(Directory),
    Empty,
}

#[derive(Clone, Default)]
pub enum TextKind {
    HELP,
    #[default]
    TEXTFILE,
}

impl Preview {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    /// Creates a new preview instance based on the filekind and the extension of
    /// the file.
    /// Sometimes it reads the content of the file, sometimes it delegates
    /// it to the display method.
    pub fn new(
        file_info: &FileInfo,
        users_cache: &Rc<UsersCache>,
        status: &Status,
    ) -> FmResult<Self> {
        match file_info.file_kind {
            FileKind::Directory => Ok(Self::Directory(Directory::new(
                &file_info.path,
                users_cache,
                &status.config_colors,
                &status.selected_non_mut().filter,
                status.selected_non_mut().show_hidden,
            )?)),
            FileKind::NormalFile => match file_info.extension.to_lowercase().as_str() {
                e if is_ext_zip(e) => Ok(Self::Archive(ZipContent::new(&file_info.path)?)),
                e if is_ext_pdf(e) => Ok(Self::Pdf(PdfContent::new(&file_info.path))),
                e if is_ext_image(e) => Ok(Self::Exif(ExifContent::new(&file_info.path)?)),
                e if is_ext_media(e) => Ok(Self::Media(MediaContent::new(&file_info.path)?)),
                e => match Self::preview_syntaxed(e, &file_info.path) {
                    Some(syntaxed_preview) => Ok(syntaxed_preview),
                    None => Self::preview_text_or_binary(file_info),
                },
            },
            _ => Err(FmError::custom(
                "new preview",
                "Can't preview this filekind",
            )),
        }
    }

    fn preview_syntaxed(ext: &str, path: &Path) -> Option<Self> {
        let ss = SyntaxSet::load_defaults_nonewlines();
        ss.find_syntax_by_extension(ext).map(|syntax| {
            Self::Syntaxed(HLContent::new(path, ss.clone(), syntax).unwrap_or_default())
        })
    }

    fn preview_text_or_binary(file_info: &FileInfo) -> FmResult<Self> {
        let mut file = std::fs::File::open(file_info.path.clone())?;
        let mut buffer = vec![0; Self::CONTENT_INSPECTOR_MIN_SIZE];
        if Self::is_binary(file_info, &mut file, &mut buffer) {
            Ok(Self::Binary(BinaryContent::new(file_info)?))
        } else {
            Ok(Self::Text(TextContent::from_file(&file_info.path)?))
        }
    }

    fn is_binary(file_info: &FileInfo, file: &mut std::fs::File, buffer: &mut [u8]) -> bool {
        file_info.size >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
            && file.read_exact(buffer).is_ok()
            && inspect(buffer) == ContentType::BINARY
    }

    /// Creates a thumbnail preview of the file.
    pub fn thumbnail(path: PathBuf) -> FmResult<Self> {
        Ok(Self::Thumbnail(Pixels::new(path)?))
    }

    /// Creates the help preview as if it was a text file.
    pub fn help(help: &str) -> Self {
        Self::Text(TextContent::help(help))
    }

    /// Empty preview, holding nothing.
    pub fn new_empty() -> Self {
        Self::Empty
    }

    /// The size (most of the time the number of lines) of the preview.
    /// Some preview (thumbnail, empty) can't be scrolled and their size is always 0.
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Syntaxed(syntaxed) => syntaxed.len(),
            Self::Text(text) => text.len(),
            Self::Binary(binary) => binary.len(),
            Self::Pdf(pdf) => pdf.len(),
            Self::Archive(zip) => zip.len(),
            Self::Thumbnail(_img) => 0,
            Self::Exif(exif_content) => exif_content.len(),
            Self::Media(media) => media.len(),
            Self::Directory(directory) => directory.len(),
        }
    }

    /// True if nothing is currently previewed.
    pub fn is_empty(&self) -> bool {
        matches!(*self, Self::Empty)
    }
}

/// Holds a preview of a text content.
/// It's a boxed vector of strings (per line)
#[derive(Clone, Default)]
pub struct TextContent {
    pub kind: TextKind,
    content: Vec<String>,
    length: usize,
}

impl TextContent {
    fn help(help: &str) -> Self {
        let content: Vec<String> = help.split('\n').map(|s| s.to_owned()).collect();
        Self {
            kind: TextKind::HELP,
            length: content.len(),
            content,
        }
    }

    fn from_file(path: &Path) -> FmResult<Self> {
        let reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let content: Vec<String> = reader
            .lines()
            .map(|line| line.unwrap_or_else(|_| "".to_owned()))
            .collect();
        Ok(Self {
            kind: TextKind::TEXTFILE,
            length: content.len(),
            content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }
}

/// Holds a preview of a code text file whose language is supported by `Syntect`.
/// The file is colored propery and line numbers are shown.
#[derive(Clone, Default)]
pub struct HLContent {
    content: Vec<Vec<SyntaxedString>>,
    length: usize,
}

impl HLContent {
    /// Creates a new displayable content of a syntect supported file.
    /// It may file if the file isn't properly formatted or the extension
    /// is wrong (ie. python content with .c extension).
    /// ATM only Solarized (dark) theme is supported.
    fn new(path: &Path, syntax_set: SyntaxSet, syntax_ref: &SyntaxReference) -> FmResult<Self> {
        let reader = std::io::BufReader::new(std::fs::File::open(path)?);
        let raw_content: Vec<String> = reader
            .lines()
            .map(|line| line.unwrap_or_else(|_| "".to_owned()))
            .collect();
        let highlighted_content = Self::parse_raw_content(raw_content, syntax_set, syntax_ref);

        Ok(Self {
            length: highlighted_content.len(),
            content: highlighted_content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }

    fn parse_raw_content(
        raw_content: Vec<String>,
        syntax_set: SyntaxSet,
        syntax_ref: &SyntaxReference,
    ) -> Vec<Vec<SyntaxedString>> {
        let theme_set = ThemeSet::load_defaults();
        let mut highlighted_content = vec![];
        let mut highlighter =
            HighlightLines::new(syntax_ref, &theme_set.themes["Solarized (dark)"]);

        for line in raw_content.iter() {
            let mut col = 0;
            let mut v_line = vec![];
            if let Ok(v) = highlighter.highlight_line(line, &syntax_set) {
                for (style, token) in v.iter() {
                    v_line.push(SyntaxedString::from_syntect(col, token, *style));
                    col += token.len();
                }
            }
            highlighted_content.push(v_line)
        }

        highlighted_content
    }
}

/// Holds a string to be displayed with given colors.
/// We have to read the colors from Syntect and parse it into tuikit attr
/// This struct does the parsing.
#[derive(Clone)]
pub struct SyntaxedString {
    // row: usize,
    col: usize,
    content: String,
    attr: Attr,
}

impl SyntaxedString {
    /// Parse a content and style into a `SyntaxedString`
    /// Only the foreground color is read, we don't the background nor
    /// the style (bold, italic, underline) defined in Syntect.
    fn from_syntect(col: usize, content: &str, style: Style) -> Self {
        let content = content.to_owned();
        let fg = style.foreground;
        let attr = Attr::from(Color::Rgb(fg.r, fg.g, fg.b));
        Self { col, content, attr }
    }

    /// Prints itself on a tuikit canvas.
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

/// Holds a preview of a binary content.
/// It doesn't try to respect endianness.
/// The lines are formatted to display 16 bytes.
#[derive(Clone)]
pub struct BinaryContent {
    pub path: PathBuf,
    length: u64,
    content: Vec<Line>,
}

impl BinaryContent {
    const LINE_WIDTH: usize = 16;

    fn new(file_info: &FileInfo) -> FmResult<Self> {
        let mut reader = BufReader::new(std::fs::File::open(file_info.path.clone())?);
        let mut buffer = [0; Self::LINE_WIDTH];
        let mut content: Vec<Line> = vec![];
        while let Ok(nb_bytes_read) = reader.read(&mut buffer[..]) {
            if nb_bytes_read != Self::LINE_WIDTH {
                content.push(Line::new((&buffer[0..nb_bytes_read]).into()));
                break;
            } else {
                content.push(Line::new(buffer.into()));
            }
        }

        Ok(Self {
            path: file_info.path.clone(),
            length: file_info.size / Self::LINE_WIDTH as u64,
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
    /// It tries to imitates the output of hexdump.
    pub fn print(&self, canvas: &mut dyn tuikit::canvas::Canvas, row: usize, offset: usize) {
        let _ = canvas.print(row, offset + 2, &self.format());
    }
}

/// Holds a preview of a pdffile as outputed by `pdf_extract` crate.
/// If the pdf file content can't be extracted, it doesn't fail but simply hold
/// an error message to be displayed.
/// Afterall, it's just a TUI filemanager, the user shouldn't expect to display
/// any kind of graphical pdf...
#[derive(Clone)]
pub struct PdfContent {
    length: usize,
    content: Vec<String>,
}

impl PdfContent {
    fn new(path: &Path) -> Self {
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

/// Holds a list of file of an archive as returned by `compress_tools::list_files`.
/// It may fail if the archive can't be read properly.
#[derive(Clone)]
pub struct ZipContent {
    length: usize,
    content: Vec<String>,
}

impl ZipContent {
    fn new(path: &Path) -> FmResult<Self> {
        let content = list_files(path)?;

        Ok(Self {
            length: content.len(),
            content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }
}

/// Holds the exif content of an image.
/// Since displaying a thumbnail is ugly and idk how to bind ueberzug into
/// tuikit, it's preferable.
/// At least it's an easy way to display informations about an image.
#[derive(Clone)]
pub struct ExifContent {
    length: usize,
    /// The exif strings.
    content: Vec<String>,
}

impl ExifContent {
    fn new(path: &Path) -> FmResult<Self> {
        let mut bufreader = BufReader::new(std::fs::File::open(path)?);
        let content: Vec<String> =
            if let Ok(exif) = exif::Reader::new().read_from_container(&mut bufreader) {
                exif.fields()
                    .map(|f| Self::format_exif_field(f, &exif))
                    .collect()
            } else {
                vec![]
            };
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

/// Holds media info about a "media" file (mostly videos and audios).
/// Requires the [`mediainfo`](https://mediaarea.net/) executable installed in path.
#[derive(Clone)]
pub struct MediaContent {
    length: usize,
    /// The media info details.
    content: Vec<String>,
}

impl MediaContent {
    fn new(path: &Path) -> FmResult<Self> {
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

/// Holds a path to an image and a method to convert it into an ugly thumbnail.
#[derive(Clone)]
pub struct Pixels {
    pub img_path: PathBuf,
}

impl Pixels {
    /// Creates a new preview instance. It simply holds a path.
    fn new(img_path: PathBuf) -> FmResult<Self> {
        Ok(Self { img_path })
    }

    /// Tries to scale down the image to be displayed in the terminal canvas.
    /// Fastest algorithm is used (nearest neighbor) since the result is always
    /// ugly nonetheless.
    /// It may be fun to show to non geek users :)
    pub fn resized_rgb8(&self, width: u32, height: u32) -> FmResult<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        let img = image::open(&self.img_path)?;
        Ok(img.resize(width, height, FilterType::Nearest).to_rgb8())
    }
}

/// Display a tree view of a directory.
/// The "tree view" is calculated recursively. It may take some time
/// if the directory has a lot of children.
#[derive(Clone, Debug)]
pub struct Directory {
    pub content: Vec<(String, ColoredString)>,
    pub tree: Tree,
    len: usize,
    pub selected_index: usize,
}

impl Directory {
    /// Creates a new tree view of the directory.
    /// We only hold the result here, since the tree itself has now usage atm.
    ///
    /// TODO! make it really navigable as other views.
    pub fn new(
        path: &Path,
        users_cache: &Rc<UsersCache>,
        colors: &Colors,
        filter_kind: &FilterKind,
        show_hidden: bool,
    ) -> FmResult<Self> {
        let mut tree =
            Tree::from_path(path, Tree::MAX_DEPTH, users_cache, filter_kind, show_hidden)?;
        tree.select_root();
        let (selected_index, content) = tree.into_navigable_content(colors);
        Ok(Self {
            tree,
            len: content.len(),
            content,
            selected_index,
        })
    }

    pub fn empty(path: &Path, users_cache: &Rc<UsersCache>) -> FmResult<Self> {
        Ok(Self {
            tree: Tree::empty(path, users_cache)?,
            len: 0,
            content: vec![],
            selected_index: 0,
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn select_root(&mut self, colors: &Colors) -> FmResult<()> {
        self.tree.select_root();
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    pub fn unselect_children(&mut self) {
        self.tree.unselect_children()
    }

    pub fn select_next_sibling(&mut self, colors: &Colors) -> FmResult<()> {
        self.tree.select_next_sibling()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    pub fn select_prev_sibling(&mut self, colors: &Colors) -> FmResult<()> {
        self.tree.select_prev_sibling()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    pub fn select_first_child(&mut self, colors: &Colors) -> FmResult<()> {
        self.tree.select_first_child()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    pub fn select_parent(&mut self, colors: &Colors) -> FmResult<()> {
        self.tree.select_parent()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    pub fn go_to_bottom_leaf(&mut self, colors: &Colors) -> FmResult<()> {
        self.tree.go_to_bottom_leaf()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    pub fn make_preview(&mut self, colors: &Colors) {
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
    }

    pub fn calculate_tree_window(&self, height: usize) -> (usize, usize, usize) {
        let length = self.content.len();
        let mut top = if self.selected_index < height {
            0
        } else {
            self.selected_index
        };
        let mut bottom = if self.selected_index < height {
            height
        } else {
            self.selected_index + height
        };

        let padding = std::cmp::max(10, height / 2);
        if self.selected_index > height {
            top -= padding;
            bottom += padding;
        }

        (top, bottom, length)
    }
}

/// Common trait for many preview methods which are just a bunch of lines with
/// no specific formatting.
/// Some previewing (thumbnail and syntaxed text) needs more details.
pub trait Window<T> {
    fn window(
        &self,
        top: usize,
        bottom: usize,
        length: usize,
    ) -> Take<Skip<Enumerate<Iter<'_, T>>>>;
}

impl Window<Vec<SyntaxedString>> for HLContent {
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

type ColoredPair = (String, ColoredString);

impl_window!(TextContent, String);
impl_window!(BinaryContent, Line);
impl_window!(PdfContent, String);
impl_window!(ZipContent, String);
impl_window!(ExifContent, String);
impl_window!(MediaContent, String);
impl_window!(Directory, ColoredPair);

fn is_ext_zip(ext: &str) -> bool {
    matches!(
        ext,
        "zip" | "gzip" | "bzip2" | "xz" | "lzip" | "lzma" | "tar" | "mtree" | "raw" | "7z"
    )
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

fn is_ext_pdf(ext: &str) -> bool {
    ext == "pdf"
}

fn catch_unwind_silent<F: FnOnce() -> R + panic::UnwindSafe, R>(f: F) -> std::thread::Result<R> {
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(f);
    panic::set_hook(prev_hook);
    result
}
