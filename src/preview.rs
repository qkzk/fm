use std::cmp::min;
use std::fmt::Write as _;
use std::fs::metadata;
use std::io::Cursor;
use std::io::{BufRead, BufReader, Read};
use std::iter::{Enumerate, Skip, Take};
use std::panic;
use std::path::{Path, PathBuf};
use std::slice::Iter;

use anyhow::{anyhow, Context, Result};
use content_inspector::{inspect, ContentType};
use log::info;
use pdf_extract;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tuikit::attr::{Attr, Color};
use users::UsersCache;

use crate::config::Colors;
use crate::constant_strings_paths::{
    DIFF, ISOINFO, JUPYTER, MEDIAINFO, PANDOC, THUMBNAIL_PATH, UEBERZUG,
};
use crate::content_window::ContentWindow;
use crate::decompress::list_files_zip;
use crate::fileinfo::{FileInfo, FileKind};
use crate::filter::FilterKind;
use crate::opener::execute_and_capture_output_without_check;
use crate::status::Status;
use crate::tree::{ColoredString, Tree};
use crate::utils::{filename_from_path, is_program_in_path};

/// Different kind of preview used to display some informaitons
/// About the file.
/// We check if it's an archive first, then a pdf file, an image, a media file
#[derive(Default)]
pub enum Preview {
    Syntaxed(HLContent),
    Text(TextContent),
    Binary(BinaryContent),
    Pdf(PdfContent),
    Archive(ZipContent),
    Ueberzug(Ueberzug),
    Media(MediaContent),
    Directory(Directory),
    Iso(Iso),
    Diff(Diff),
    ColoredText(ColoredText),
    #[default]
    Empty,
}

#[derive(Clone, Default)]
pub enum TextKind {
    HELP,
    LOG,
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
        users_cache: &UsersCache,
        status: &Status,
        colors: &Colors,
    ) -> Result<Self> {
        match file_info.file_kind {
            FileKind::Directory => Ok(Self::Directory(Directory::new(
                &file_info.path,
                users_cache,
                colors,
                &status.selected_non_mut().filter,
                status.selected_non_mut().show_hidden,
                Some(2),
            )?)),
            FileKind::NormalFile => match file_info.extension.to_lowercase().as_str() {
                e if is_ext_compressed(e) => Ok(Self::Archive(ZipContent::new(&file_info.path)?)),
                e if is_ext_pdf(e) => Ok(Self::Pdf(PdfContent::new(&file_info.path))),
                e if is_ext_image(e) && is_program_in_path(UEBERZUG) => {
                    Ok(Self::Ueberzug(Ueberzug::image(&file_info.path)?))
                }
                e if is_ext_audio(e) && is_program_in_path(MEDIAINFO) => {
                    Ok(Self::Media(MediaContent::new(&file_info.path)?))
                }
                e if is_ext_video(e) && is_program_in_path(UEBERZUG) => {
                    Ok(Self::Ueberzug(Ueberzug::video_thumbnail(&file_info.path)?))
                }
                e if is_ext_font(e) && is_program_in_path(UEBERZUG) => {
                    Ok(Self::Ueberzug(Ueberzug::font_thumbnail(&file_info.path)?))
                }
                e if is_ext_svg(e) && is_program_in_path(UEBERZUG) => {
                    Ok(Self::Ueberzug(Ueberzug::svg_thumbnail(&file_info.path)?))
                }
                e if is_ext_iso(e) && is_program_in_path(ISOINFO) => {
                    Ok(Self::Iso(Iso::new(&file_info.path)?))
                }
                e if is_ext_notebook(e) && is_program_in_path(JUPYTER) => {
                    Ok(Self::notebook(&file_info.path)
                        .context("Preview: Couldn't parse notebook")?)
                }
                e if is_ext_doc(e) && is_program_in_path(PANDOC) => {
                    Ok(Self::doc(&file_info.path).context("Preview: Couldn't parse doc")?)
                }
                e => match Self::preview_syntaxed(e, &file_info.path) {
                    Some(syntaxed_preview) => Ok(syntaxed_preview),
                    None => Self::preview_text_or_binary(file_info),
                },
            },
            _ => Err(anyhow!("new preview: can't preview this filekind",)),
        }
    }

    /// Creates a new, static window used when we display a preview in the second pane
    pub fn window_for_second_pane(&self, height: usize) -> ContentWindow {
        ContentWindow::new(self.len(), height)
    }

    fn preview_syntaxed(ext: &str, path: &Path) -> Option<Self> {
        if let Ok(metadata) = metadata(path) {
            if metadata.len() > HLContent::SIZE_LIMIT as u64 {
                return None;
            }
        } else {
            return None;
        };
        let ss = SyntaxSet::load_defaults_nonewlines();
        ss.find_syntax_by_extension(ext).map(|syntax| {
            Self::Syntaxed(HLContent::new(path, ss.clone(), syntax).unwrap_or_default())
        })
    }

    fn notebook(path: &Path) -> Option<Self> {
        let path_str = path.to_str()?;
        // nbconvert is bundled with jupyter, no need to check again
        let output = execute_and_capture_output_without_check(
            JUPYTER,
            &["nbconvert", "--to", "markdown", path_str, "--stdout"],
        )
        .ok()?;
        Self::syntaxed_from_str(output, "md")
    }

    fn doc(path: &Path) -> Option<Self> {
        let path_str = path.to_str()?;
        let output = execute_and_capture_output_without_check(
            PANDOC,
            &["-s", "-t", "markdown", "--", path_str],
        )
        .ok()?;
        Self::syntaxed_from_str(output, "md")
    }

    fn syntaxed_from_str(output: String, ext: &str) -> Option<Self> {
        let ss = SyntaxSet::load_defaults_nonewlines();
        ss.find_syntax_by_extension(ext).map(|syntax| {
            Self::Syntaxed(HLContent::from_str(&output, ss.clone(), syntax).unwrap_or_default())
        })
    }

    fn preview_text_or_binary(file_info: &FileInfo) -> Result<Self> {
        let mut file = std::fs::File::open(file_info.path.clone())?;
        let mut buffer = vec![0; Self::CONTENT_INSPECTOR_MIN_SIZE];
        if Self::is_binary(file_info, &mut file, &mut buffer) {
            Ok(Self::Binary(BinaryContent::new(file_info)?))
        } else {
            Ok(Self::Text(TextContent::from_file(&file_info.path)?))
        }
    }

    fn is_binary(file_info: &FileInfo, file: &mut std::fs::File, buffer: &mut [u8]) -> bool {
        file_info.size().unwrap_or_default() >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
            && file.read_exact(buffer).is_ok()
            && inspect(buffer) == ContentType::BINARY
    }

    /// Returns mediainfo of a media file.
    pub fn mediainfo(path: &Path) -> Result<Self> {
        Ok(Self::Media(MediaContent::new(path)?))
    }

    pub fn diff(first_path: &str, second_path: &str) -> Result<Self> {
        Ok(Self::Diff(Diff::new(first_path, second_path)?))
    }

    /// Creates the help preview as if it was a text file.
    pub fn help(help: &str) -> Self {
        Self::Text(TextContent::help(help))
    }

    pub fn log(log: Vec<String>) -> Self {
        Self::Text(TextContent::log(log))
    }

    pub fn cli_info(output: &str) -> Self {
        Self::ColoredText(ColoredText::new(output))
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
            Self::Ueberzug(_img) => 0,
            Self::Media(media) => media.len(),
            Self::Directory(directory) => directory.len(),
            Self::Diff(diff) => diff.len(),
            Self::Iso(iso) => iso.len(),
            Self::ColoredText(text) => text.len(),
        }
    }

    /// True if nothing is currently previewed.
    pub fn is_empty(&self) -> bool {
        matches!(*self, Self::Empty)
    }
}

/// Read a number of lines from a text file. Returns a vector of strings.
fn read_nb_lines(path: &Path, size_limit: usize) -> Result<Vec<String>> {
    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    Ok(reader
        .lines()
        .take(size_limit)
        .map(|line| line.unwrap_or_else(|_| "".to_owned()))
        .collect())
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
    const SIZE_LIMIT: usize = 1048576;

    fn help(help: &str) -> Self {
        let content: Vec<String> = help.split('\n').map(|s| s.to_owned()).collect();
        Self {
            kind: TextKind::HELP,
            length: content.len(),
            content,
        }
    }

    fn log(content: Vec<String>) -> Self {
        Self {
            kind: TextKind::LOG,
            length: content.len(),
            content,
        }
    }

    fn from_file(path: &Path) -> Result<Self> {
        let content = read_nb_lines(path, Self::SIZE_LIMIT)?;
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
    const SIZE_LIMIT: usize = 32768;
    /// Creates a new displayable content of a syntect supported file.
    /// It may file if the file isn't properly formatted or the extension
    /// is wrong (ie. python content with .c extension).
    /// ATM only Solarized (dark) theme is supported.
    fn new(path: &Path, syntax_set: SyntaxSet, syntax_ref: &SyntaxReference) -> Result<Self> {
        let raw_content = read_nb_lines(path, Self::SIZE_LIMIT)?;
        let highlighted_content = Self::parse_raw_content(raw_content, syntax_set, syntax_ref)?;

        Ok(Self {
            length: highlighted_content.len(),
            content: highlighted_content,
        })
    }

    fn from_str(text: &str, syntax_set: SyntaxSet, syntax_ref: &SyntaxReference) -> Result<Self> {
        let raw_content = text.lines().map(|s| s.to_owned()).collect();
        let highlighted_content = Self::parse_raw_content(raw_content, syntax_set, syntax_ref)?;
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
    ) -> Result<Vec<Vec<SyntaxedString>>> {
        let mut monokai = BufReader::new(Cursor::new(include_bytes!(
            "../assets/themes/Monokai_Extended.tmTheme"
        )));
        let theme = ThemeSet::load_from_reader(&mut monokai)?;
        let mut highlighted_content = vec![];
        let mut highlighter = HighlightLines::new(syntax_ref, &theme);

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

        Ok(highlighted_content)
    }
}

/// Holds a string to be displayed with given colors.
/// We have to read the colors from Syntect and parse it into tuikit attr
/// This struct does the parsing.
#[derive(Clone)]
pub struct SyntaxedString {
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
    ) -> Result<()> {
        canvas.print_with_attr(row, self.col + offset + 2, &self.content, self.attr)?;
        Ok(())
    }
}

/// Holds a preview of a binary content.
/// It doesn't try to respect endianness.
/// The lines are formatted to display 16 bytes.
/// The number of lines is truncated to $2^20 = 1048576$.
#[derive(Clone)]
pub struct BinaryContent {
    pub path: PathBuf,
    length: u64,
    content: Vec<Line>,
}

impl BinaryContent {
    const LINE_WIDTH: usize = 16;
    const SIZE_LIMIT: usize = 1048576;

    fn new(file_info: &FileInfo) -> Result<Self> {
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
            if content.len() >= Self::SIZE_LIMIT {
                break;
            }
        }

        Ok(Self {
            path: file_info.path.clone(),
            length: file_info.size().unwrap_or_default() / Self::LINE_WIDTH as u64,
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
            let _ = write!(s, "{byte:02x}");
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
    const SIZE_LIMIT: usize = 1048576;

    fn new(path: &Path) -> Self {
        let result = catch_unwind_silent(|| {
            // TODO! remove this when pdf_extract replaces println! whith dlog.
            let _print_gag = gag::Gag::stdout().unwrap();
            if let Ok(content_string) = pdf_extract::extract_text(path) {
                content_string
                    .split_whitespace()
                    .take(Self::SIZE_LIMIT)
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

/// Holds a list of file of an archive as returned by `ZipArchive::file_names`.
/// A generic error message prevent it from returning an error.
#[derive(Clone)]
pub struct ZipContent {
    length: usize,
    content: Vec<String>,
}

impl ZipContent {
    fn new(path: &Path) -> Result<Self> {
        let content = list_files_zip(path).unwrap_or(vec!["Invalid Zip content".to_owned()]);

        Ok(Self {
            length: content.len(),
            content,
        })
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
    fn new(path: &Path) -> Result<Self> {
        let content: Vec<String>;
        if let Ok(output) = std::process::Command::new(MEDIAINFO).arg(path).output() {
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

/// Holds a path, a filename and an instance of ueberzug::Ueberzug.
/// The ueberzug instance is held as long as the preview is displayed.
/// When the preview is reset, the instance is dropped and the image is erased.
/// Positonning the image is tricky since tuikit doesn't know where it's drawed in the terminal:
/// the preview can't be placed correctly in embeded terminals.
pub struct Ueberzug {
    path: String,
    filename: String,
    ueberzug: ueberzug::Ueberzug,
}

impl Ueberzug {
    fn image(img_path: &Path) -> Result<Self> {
        let filename = filename_from_path(img_path)?.to_owned();
        let path = img_path
            .to_str()
            .context("ueberzug: couldn't parse the path into a string")?
            .to_owned();
        Ok(Self {
            path,
            filename,
            ueberzug: ueberzug::Ueberzug::new(),
        })
    }

    fn thumbnail() -> Self {
        Self {
            path: THUMBNAIL_PATH.to_owned(),
            filename: "thumbnail".to_owned(),
            ueberzug: ueberzug::Ueberzug::new(),
        }
    }

    fn video_thumbnail(video_path: &Path) -> Result<Self> {
        Self::make_video_thumbnail(video_path)?;
        Ok(Self::thumbnail())
    }

    fn font_thumbnail(font_path: &Path) -> Result<Self> {
        Self::make_font_thumbnail(font_path)?;
        Ok(Self::thumbnail())
    }

    fn svg_thumbnail(svg_path: &Path) -> Result<Self> {
        Self::make_svg_thumbnail(svg_path)?;
        Ok(Self::thumbnail())
    }

    fn make_thumbnail(exe: &str, args: &[&str]) -> Result<()> {
        let output = std::process::Command::new(exe).args(args).output()?;
        if !output.stderr.is_empty() {
            info!(
                "make thumbnail output: {} {}",
                String::from_utf8(output.stdout).unwrap_or_default(),
                String::from_utf8(output.stderr).unwrap_or_default()
            );
        }
        Ok(())
    }

    fn make_video_thumbnail(video_path: &Path) -> Result<()> {
        let path_str = video_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make_thumbnail(
            "ffmpeg",
            &[
                "-i",
                path_str,
                "-vf",
                "thumbnail",
                "-frames:v",
                "1",
                THUMBNAIL_PATH,
                "-y",
            ],
        )
    }

    fn make_font_thumbnail(font_path: &Path) -> Result<()> {
        let path_str = font_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make_thumbnail("fontimage", &["-o", THUMBNAIL_PATH, path_str])
    }

    fn make_svg_thumbnail(svg_path: &Path) -> Result<()> {
        let path_str = svg_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make_thumbnail(
            "rsvg-convert",
            &["--keep-aspect-ratio", path_str, "-o", THUMBNAIL_PATH],
        )
    }

    /// Draw the image with ueberzug in the current window.
    /// The position is absolute, which is problematic when the app is embeded into a floating terminal.
    /// The whole struct instance is dropped when the preview is reset and the image is deleted.
    pub fn ueberzug(&self, x: u16, y: u16, width: u16, height: u16) {
        self.ueberzug.draw(&ueberzug::UeConf {
            identifier: &self.filename,
            path: &self.path,
            x,
            y,
            width: Some(width),
            height: Some(height),
            scaler: Some(ueberzug::Scalers::FitContain),
            ..Default::default()
        });
    }
}

#[derive(Clone, Debug)]
pub struct ColoredText {
    pub content: Vec<String>,
    len: usize,
    pub selected_index: usize,
}

impl ColoredText {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Make a new previewed colored text.
    pub fn new(output: &str) -> Self {
        let content: Vec<String> = output.lines().map(|line| line.to_owned()).collect();
        let len = content.len();
        let selected_index = 0;
        Self {
            content,
            len,
            selected_index,
        }
    }
}

/// Display a tree view of a directory.
/// The "tree view" is calculated recursively. It may take some time
/// if the directory has a lot of children.
#[derive(Clone, Debug)]
pub struct Directory {
    pub content: Vec<ColoredTriplet>,
    pub tree: Tree,
    len: usize,
    pub selected_index: usize,
}

impl Directory {
    /// Creates a new tree view of the directory.
    /// We only hold the result here, since the tree itself has now usage atm.
    pub fn new(
        path: &Path,
        users_cache: &UsersCache,
        colors: &Colors,
        filter_kind: &FilterKind,
        show_hidden: bool,
        max_depth: Option<usize>,
    ) -> Result<Self> {
        let max_depth = match max_depth {
            Some(max_depth) => max_depth,
            None => Tree::MAX_DEPTH,
        };

        let mut tree = Tree::from_path(
            path,
            max_depth,
            users_cache,
            filter_kind,
            show_hidden,
            vec![0],
        )?;
        tree.select_root();
        let (selected_index, content) = tree.into_navigable_content(colors);
        Ok(Self {
            tree,
            len: content.len(),
            content,
            selected_index,
        })
    }

    /// Creates an empty directory preview.
    pub fn empty(path: &Path, users_cache: &UsersCache) -> Result<Self> {
        Ok(Self {
            tree: Tree::empty(path, users_cache)?,
            len: 0,
            content: vec![],
            selected_index: 0,
        })
    }

    /// Reset the attributes to default one and free some unused memory.
    pub fn clear(&mut self) {
        self.len = 0;
        self.content = vec![];
        self.selected_index = 0;
        self.tree.clear();
    }

    /// Number of displayed lines.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if there's no lines in preview.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Select the root node and reset the view.
    pub fn select_root(&mut self, colors: &Colors) -> Result<()> {
        self.tree.select_root();
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    /// Unselect every child node.
    pub fn unselect_children(&mut self) {
        self.tree.unselect_children()
    }

    /// Select the "next" element of the tree if any.
    /// This is the element immediatly below the current one.
    pub fn select_next(&mut self, colors: &Colors) -> Result<()> {
        if self.selected_index < self.content.len() {
            self.tree.increase_required_height();
            self.unselect_children();
            self.selected_index += 1;
            self.update_tree_position_from_index(colors)?;
        }
        Ok(())
    }

    /// Select the previous sibling if any.
    /// This is the element immediatly below the current one.
    pub fn select_prev(&mut self, colors: &Colors) -> Result<()> {
        if self.selected_index > 0 {
            self.tree.decrease_required_height();
            self.unselect_children();
            self.selected_index -= 1;
            self.update_tree_position_from_index(colors)?;
        }
        Ok(())
    }

    /// Move up 10 times.
    pub fn page_up(&mut self, colors: &Colors) -> Result<()> {
        if self.selected_index > 10 {
            self.selected_index -= 10;
        } else {
            self.selected_index = 1;
        }
        self.update_tree_position_from_index(colors)
    }

    /// Move down 10 times
    pub fn page_down(&mut self, colors: &Colors) -> Result<()> {
        self.selected_index += 10;
        if self.selected_index > self.content.len() {
            if !self.content.is_empty() {
                self.selected_index = self.content.len();
            } else {
                self.selected_index = 1;
            }
        }
        self.update_tree_position_from_index(colors)
    }

    /// Update the position of the selected element from its index.
    pub fn update_tree_position_from_index(&mut self, colors: &Colors) -> Result<()> {
        self.tree.position = self.tree.position_from_index(self.selected_index);
        let (_, _, node) = self.tree.select_from_position()?;
        self.tree.current_node = node;
        (_, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    /// Select the first child, if any.
    pub fn select_first_child(&mut self, colors: &Colors) -> Result<()> {
        self.tree.select_first_child()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    /// Select the parent of current node.
    pub fn select_parent(&mut self, colors: &Colors) -> Result<()> {
        self.tree.select_parent()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    /// Select the last leaf of the tree (ie the last line.)
    pub fn go_to_bottom_leaf(&mut self, colors: &Colors) -> Result<()> {
        self.tree.go_to_bottom_leaf()?;
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
        Ok(())
    }

    /// Make a preview of the tree.
    pub fn make_preview(&mut self, colors: &Colors) {
        (self.selected_index, self.content) = self.tree.into_navigable_content(colors);
    }

    /// Calculates the top, bottom and lenght of the view, depending on which element
    /// is selected and the size of the window used to display.
    pub fn calculate_tree_window(&self, terminal_height: usize) -> (usize, usize, usize) {
        let length = self.content.len();

        let top: usize;
        let bottom: usize;
        let window_height = terminal_height - ContentWindow::WINDOW_MARGIN_TOP;
        if self.selected_index < terminal_height - 1 {
            top = 0;
            bottom = window_height;
        } else {
            let padding = std::cmp::max(10, terminal_height / 2);
            top = self.selected_index - padding;
            bottom = top + window_height;
        }

        (top, bottom, length)
    }
}

pub struct Diff {
    pub content: Vec<String>,
    length: usize,
}

impl Diff {
    pub fn new(first_path: &str, second_path: &str) -> Result<Self> {
        let content: Vec<String> =
            execute_and_capture_output_without_check(DIFF, &[first_path, second_path])?
                .lines()
                .map(|s| s.to_owned())
                .collect();
        info!("{DIFF}:\n{content:?}");

        Ok(Self {
            length: content.len(),
            content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }
}

pub struct Iso {
    pub content: Vec<String>,
    length: usize,
}

impl Iso {
    fn new(path: &Path) -> Result<Self> {
        let path = path.to_str().context("couldn't parse the path")?;
        let content: Vec<String> =
            execute_and_capture_output_without_check(ISOINFO, &["-l", "-i", path])?
                .lines()
                .map(|s| s.to_owned())
                .collect();
        info!("{ISOINFO}:\n{content:?}");

        Ok(Self {
            length: content.len(),
            content,
        })
    }

    fn len(&self) -> usize {
        self.length
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

/// A tuple with `(ColoredString, String, ColoredString)`.
/// Used to iter and impl window trait in tree mode.
pub type ColoredTriplet = (ColoredString, String, ColoredString);

/// A vector of highlighted strings
pub type VecSyntaxedString = Vec<SyntaxedString>;

impl_window!(HLContent, VecSyntaxedString);
impl_window!(TextContent, String);
impl_window!(BinaryContent, Line);
impl_window!(PdfContent, String);
impl_window!(ZipContent, String);
impl_window!(MediaContent, String);
impl_window!(Directory, ColoredTriplet);
impl_window!(Diff, String);
impl_window!(Iso, String);
impl_window!(ColoredText, String);

fn is_ext_compressed(ext: &str) -> bool {
    matches!(
        ext,
        "zip" | "gzip" | "bzip2" | "xz" | "lzip" | "lzma" | "tar" | "mtree" | "raw" | "7z"
    )
}

/// True iff the extension is a known (by me) image extension.
pub fn is_ext_image(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "tiff" | "heif" | "gif" | "raw" | "cr2" | "nef" | "orf" | "sr2"
    )
}

fn is_ext_audio(ext: &str) -> bool {
    matches!(
        ext,
        "ogg"
            | "ogm"
            | "riff"
            | "mp2"
            | "mp3"
            | "wm"
            | "qt"
            | "ac3"
            | "dts"
            | "aac"
            | "mac"
            | "flac"
    )
}

fn is_ext_video(ext: &str) -> bool {
    matches!(ext, "mkv" | "webm" | "mpeg" | "mp4" | "avi" | "flv" | "mpg")
}

fn is_ext_font(ext: &str) -> bool {
    ext == "ttf"
}

fn is_ext_svg(ext: &str) -> bool {
    ext == "svg"
}

fn is_ext_pdf(ext: &str) -> bool {
    ext == "pdf"
}

fn is_ext_iso(ext: &str) -> bool {
    ext == "iso"
}

fn is_ext_notebook(ext: &str) -> bool {
    ext == "ipynb"
}

fn is_ext_doc(ext: &str) -> bool {
    matches!(ext, "doc" | "docx" | "odt" | "sxw")
}

fn catch_unwind_silent<F: FnOnce() -> R + panic::UnwindSafe, R>(f: F) -> std::thread::Result<R> {
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let result = panic::catch_unwind(f);
    panic::set_hook(prev_hook);
    result
}
