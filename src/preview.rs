use std::cmp::min;
use std::fmt::Write as _;
use std::fs::metadata;
use std::io::Cursor;
use std::io::{BufRead, BufReader, Read};
use std::iter::{Enumerate, Skip, Take};
use std::path::{Path, PathBuf};
use std::slice::Iter;

use anyhow::{anyhow, Context, Result};
use content_inspector::{inspect, ContentType};
use log::info;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tuikit::attr::{Attr, Color};

use crate::constant_strings_paths::{
    CALC_PDF_PATH, DIFF, FFMPEG, FONTIMAGE, ISOINFO, JUPYTER, LIBREOFFICE, LSBLK, LSOF, MEDIAINFO,
    PANDOC, RSVG_CONVERT, SS, THUMBNAIL_PATH, UEBERZUG,
};
use crate::content_window::ContentWindow;
use crate::edit_mode::FilterKind;
use crate::edit_mode::SortKind;
use crate::edit_mode::{list_files_tar, list_files_zip};
use crate::fileinfo::{ColorEffect, FileInfo, FileKind};
use crate::opener::execute_and_capture_output_without_check;
use crate::tree::{ColoredString, Tree};
use crate::users::Users;
use crate::utils::{clear_tmp_file, filename_from_path, is_program_in_path, path_to_string};

/// Different kind of extension for grouped by previewers.
/// Any extension we can preview should be matched here.
#[derive(Default)]
pub enum ExtensionKind {
    Archive,
    Image,
    Audio,
    Video,
    Font,
    Svg,
    Pdf,
    Iso,
    Notebook,
    Office,
    Epub,
    #[default]
    Unknown,
}

impl ExtensionKind {
    /// Match any known extension against an extension kind.
    pub fn matcher(ext: &str) -> Self {
        match ext {
            "zip" | "gzip" | "bzip2" | "xz" | "lzip" | "lzma" | "tar" | "mtree" | "raw" | "7z"
            | "gz" | "zst" => Self::Archive,
            "png" | "jpg" | "jpeg" | "tiff" | "heif" | "gif" | "cr2" | "nef" | "orf" | "sr2" => {
                Self::Image
            }
            "ogg" | "ogm" | "riff" | "mp2" | "mp3" | "wm" | "qt" | "ac3" | "dts" | "aac"
            | "mac" | "flac" => Self::Audio,
            "mkv" | "webm" | "mpeg" | "mp4" | "avi" | "flv" | "mpg" => Self::Video,
            "ttf" | "otf" => Self::Font,
            "svg" | "svgz" => Self::Svg,
            "pdf" => Self::Pdf,
            "iso" => Self::Iso,
            "ipynb" => Self::Notebook,
            "doc" | "docx" | "odt" | "sxw" | "xlsx" | "xls" => Self::Office,
            "epub" => Self::Epub,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Default)]
pub enum TextKind {
    HELP,
    LOG,
    EPUB,
    #[default]
    TEXTFILE,
}

/// Different kind of preview used to display some informaitons
/// About the file.
/// We check if it's an archive first, then a pdf file, an image, a media file
#[derive(Default)]
pub enum Preview {
    Syntaxed(HLContent),
    Text(TextContent),
    Binary(BinaryContent),
    Archive(ArchiveContent),
    Ueberzug(Ueberzug),
    Media(MediaContent),
    Directory(Directory),
    Iso(Iso),
    Diff(Diff),
    ColoredText(ColoredText),
    Socket(Socket),
    BlockDevice(BlockDevice),
    FifoCharDevice(FifoCharDevice),
    #[default]
    Empty,
}

impl Preview {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    /// Empty preview, holding nothing.
    pub fn empty() -> Self {
        clear_tmp_file();
        Self::Empty
    }

    /// Creates a new `Directory` from the file_info
    /// It explores recursivelly the directory and creates a tree.
    /// The recursive exploration is limited to depth 2.
    pub fn directory(file_info: &FileInfo, users: &Users) -> Result<Self> {
        Ok(Self::Directory(Directory::new(
            file_info.path.to_owned(),
            users,
        )))
    }

    /// Creates a new preview instance based on the filekind and the extension of
    /// the file.
    /// Sometimes it reads the content of the file, sometimes it delegates
    /// it to the display method.
    /// Directories aren't handled there since we need more arguments to create
    /// their previews.
    pub fn file(file_info: &FileInfo) -> Result<Self> {
        clear_tmp_file();
        match file_info.file_kind {
            FileKind::Directory => Err(anyhow!(
                "{path} is a directory",
                path = file_info.path.display()
            )),
            FileKind::NormalFile => {
                let extension = &file_info.extension.to_lowercase();
                match ExtensionKind::matcher(extension) {
                    ExtensionKind::Archive => Ok(Self::Archive(ArchiveContent::new(
                        &file_info.path,
                        extension,
                    )?)),
                    ExtensionKind::Pdf => Ok(Self::Ueberzug(Ueberzug::make(
                        &file_info.path,
                        UeberzugKind::Pdf,
                    )?)),
                    ExtensionKind::Image if is_program_in_path(UEBERZUG) => Ok(Self::Ueberzug(
                        Ueberzug::make(&file_info.path, UeberzugKind::Image)?,
                    )),
                    ExtensionKind::Audio if is_program_in_path(MEDIAINFO) => {
                        Ok(Self::Media(MediaContent::new(&file_info.path)?))
                    }
                    ExtensionKind::Video
                        if is_program_in_path(UEBERZUG) && is_program_in_path(FFMPEG) =>
                    {
                        Ok(Self::Ueberzug(Ueberzug::make(
                            &file_info.path,
                            UeberzugKind::Video,
                        )?))
                    }
                    ExtensionKind::Font
                        if is_program_in_path(UEBERZUG) && is_program_in_path(FONTIMAGE) =>
                    {
                        Ok(Self::Ueberzug(Ueberzug::make(
                            &file_info.path,
                            UeberzugKind::Font,
                        )?))
                    }
                    ExtensionKind::Svg
                        if is_program_in_path(UEBERZUG) && is_program_in_path(RSVG_CONVERT) =>
                    {
                        Ok(Self::Ueberzug(Ueberzug::make(
                            &file_info.path,
                            UeberzugKind::Svg,
                        )?))
                    }
                    ExtensionKind::Iso if is_program_in_path(ISOINFO) => {
                        Ok(Self::Iso(Iso::new(&file_info.path)?))
                    }
                    ExtensionKind::Notebook if is_program_in_path(JUPYTER) => {
                        Ok(Self::notebook(&file_info.path)
                            .context("Preview: Couldn't parse notebook")?)
                    }
                    ExtensionKind::Office if is_program_in_path(LIBREOFFICE) => Ok(Self::Ueberzug(
                        Ueberzug::make(&file_info.path, UeberzugKind::Office)?,
                    )),
                    ExtensionKind::Epub if is_program_in_path(PANDOC) => {
                        Ok(Self::epub(&file_info.path).context("Preview: Couldn't parse epub")?)
                    }
                    _ => match Self::preview_syntaxed(extension, &file_info.path) {
                        Some(syntaxed_preview) => Ok(syntaxed_preview),
                        None => Self::preview_text_or_binary(file_info),
                    },
                }
            }
            FileKind::Socket if is_program_in_path(SS) => Ok(Self::socket(file_info)),
            FileKind::BlockDevice if is_program_in_path(LSBLK) => Ok(Self::blockdevice(file_info)),
            FileKind::Fifo | FileKind::CharDevice if is_program_in_path(LSOF) => {
                Ok(Self::fifo_chardevice(file_info))
            }
            _ => Err(anyhow!("new preview: can't preview this filekind",)),
        }
    }

    fn socket(file_info: &FileInfo) -> Self {
        Self::Socket(Socket::new(file_info))
    }

    fn blockdevice(file_info: &FileInfo) -> Self {
        Self::BlockDevice(BlockDevice::new(file_info))
    }

    fn fifo_chardevice(file_info: &FileInfo) -> Self {
        Self::FifoCharDevice(FifoCharDevice::new(file_info))
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
        file_info.true_size >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
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

    pub fn epub(path: &Path) -> Result<Self> {
        Ok(Self::Text(
            TextContent::epub(path).context("Couldn't read epub")?,
        ))
    }

    /// The size (most of the time the number of lines) of the preview.
    /// Some preview (thumbnail, empty) can't be scrolled and their size is always 0.
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Syntaxed(syntaxed) => syntaxed.len(),
            Self::Text(text) => text.len(),
            Self::Binary(binary) => binary.len(),
            Self::Archive(zip) => zip.len(),
            Self::Ueberzug(ueberzug) => ueberzug.len(),
            Self::Media(media) => media.len(),
            Self::Directory(directory) => directory.len(),
            Self::Diff(diff) => diff.len(),
            Self::Iso(iso) => iso.len(),
            Self::ColoredText(text) => text.len(),
            Self::Socket(socket) => socket.len(),
            Self::BlockDevice(blockdevice) => blockdevice.len(),
            Self::FifoCharDevice(fifo) => fifo.len(),
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

/// Preview a socket file with `ss -lpmepiT`
#[derive(Clone, Default)]
pub struct Socket {
    content: Vec<String>,
    length: usize,
}

impl Socket {
    /// New socket preview
    /// See `man ss` for a description of the arguments.
    fn new(fileinfo: &FileInfo) -> Self {
        let content: Vec<String>;
        if let Ok(output) = std::process::Command::new(SS).arg("-lpmepiT").output() {
            let s = String::from_utf8(output.stdout).unwrap_or_default();
            content = s
                .lines()
                .filter(|l| l.contains(&fileinfo.filename))
                .map(|s| s.to_owned())
                .collect();
        } else {
            content = vec![];
        }
        Self {
            length: content.len(),
            content,
        }
    }

    fn len(&self) -> usize {
        self.length
    }
}

/// Preview a blockdevice file with lsblk
#[derive(Clone, Default)]
pub struct BlockDevice {
    content: Vec<String>,
    length: usize,
}

impl BlockDevice {
    /// New socket preview
    /// See `man ss` for a description of the arguments.
    fn new(fileinfo: &FileInfo) -> Self {
        let content: Vec<String>;
        if let Ok(output) = std::process::Command::new(LSBLK)
            .args([
                "-lfo",
                "FSTYPE,PATH,LABEL,UUID,FSVER,MOUNTPOINT,MODEL,SIZE,FSAVAIL,FSUSE%",
                &path_to_string(&fileinfo.path),
            ])
            .output()
        {
            let s = String::from_utf8(output.stdout).unwrap_or_default();
            content = s.lines().map(|s| s.to_owned()).collect();
        } else {
            content = vec![];
        }
        Self {
            length: content.len(),
            content,
        }
    }

    fn len(&self) -> usize {
        self.length
    }
}

/// Preview a fifo or a chardevice file with lsof
#[derive(Clone, Default)]
pub struct FifoCharDevice {
    content: Vec<String>,
    length: usize,
}

impl FifoCharDevice {
    /// New FIFO preview
    /// See `man lsof` for a description of the arguments.
    fn new(fileinfo: &FileInfo) -> Self {
        let content: Vec<String>;
        if let Ok(output) = std::process::Command::new(LSOF)
            .arg(path_to_string(&fileinfo.path))
            .output()
        {
            let s = String::from_utf8(output.stdout).unwrap_or_default();
            content = s.lines().map(|s| s.to_owned()).collect();
        } else {
            content = vec![];
        }
        Self {
            length: content.len(),
            content,
        }
    }

    fn len(&self) -> usize {
        self.length
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
    const SIZE_LIMIT: usize = 1048576;

    fn help(help: &str) -> Self {
        let content: Vec<String> = help.lines().map(|line| line.to_owned()).collect();
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

    fn epub(path: &Path) -> Option<Self> {
        let path_str = path.to_str()?;
        let output = execute_and_capture_output_without_check(
            PANDOC,
            &["-s", "-t", "plain", "--", path_str],
        )
        .ok()?;
        let content: Vec<String> = output.lines().map(|line| line.to_owned()).collect();
        Some(Self {
            kind: TextKind::EPUB,
            length: content.len(),
            content,
        })
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

/// Holds a string to be displayed with given .
/// We have to read the  from Syntect and parse it into tuikit attr
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
            length: file_info.true_size / Self::LINE_WIDTH as u64,
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

    /// Format a line of 16 bytes as BigEndian, separated by spaces.
    /// Every byte is zero filled if necessary.
    fn format_hex(&self) -> String {
        let mut s = String::new();
        for (i, byte) in self.line.iter().enumerate() {
            let _ = write!(s, "{byte:02x}");
            if i % 2 == 1 {
                s.push(' ');
            }
        }
        s
    }

    /// Format a line of 16 bytes as an ASCII string.
    /// Non ASCII printable bytes are replaced by dots.
    fn format_as_ascii(&self) -> String {
        let mut line_of_char = String::new();
        for byte in self.line.iter() {
            if *byte < 31 || *byte > 126 {
                line_of_char.push('.')
            } else if let Some(c) = char::from_u32(*byte as u32) {
                line_of_char.push(c);
            } else {
                line_of_char.push(' ')
            }
        }
        line_of_char
    }

    /// Print line of pair of bytes in hexadecimal, 16 bytes long.
    /// It uses BigEndian notation, regardless of platform usage.
    /// It tries to imitates the output of hexdump.
    pub fn print_bytes(&self, canvas: &mut dyn tuikit::canvas::Canvas, row: usize, offset: usize) {
        let _ = canvas.print(row, offset + 2, &self.format_hex());
    }

    /// Print a line as an ASCII string
    /// Non ASCII printable bytes are replaced by dots.
    pub fn print_ascii(&self, canvas: &mut dyn tuikit::canvas::Canvas, row: usize, offset: usize) {
        let _ = canvas.print_with_attr(
            row,
            offset + 2,
            &self.format_as_ascii(),
            Attr {
                fg: Color::LIGHT_YELLOW,
                ..Default::default()
            },
        );
    }
}

/// Holds a list of file of an archive as returned by
/// `ZipArchive::file_names` or from  a `tar tvf` command.
/// A generic error message prevent it from returning an error.
#[derive(Clone)]
pub struct ArchiveContent {
    length: usize,
    content: Vec<String>,
}

impl ArchiveContent {
    fn new(path: &Path, ext: &str) -> Result<Self> {
        let content = match ext {
            "zip" => list_files_zip(path).unwrap_or(vec!["Invalid Zip content".to_owned()]),
            "zst" | "gz" | "bz" | "xz" | "gzip" | "bzip2" => {
                list_files_tar(path).unwrap_or(vec!["Invalid Tar content".to_owned()])
            }
            _ => vec![format!("Unsupported format: {ext}")],
        };

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

pub enum UeberzugKind {
    Font,
    Image,
    Office,
    Pdf,
    Svg,
    Video,
}

/// Holds a path, a filename and an instance of ueberzug::Ueberzug.
/// The ueberzug instance is held as long as the preview is displayed.
/// When the preview is reset, the instance is dropped and the image is erased.
/// Positonning the image is tricky since tuikit doesn't know where it's drawed in the terminal:
/// the preview can't be placed correctly in embeded terminals.
pub struct Ueberzug {
    original: PathBuf,
    path: String,
    filename: String,
    kind: UeberzugKind,
    ueberzug: ueberzug::Ueberzug,
    length: usize,
    pub index: usize,
}

impl Ueberzug {
    fn thumbnail(original: PathBuf, kind: UeberzugKind) -> Self {
        Self {
            original,
            path: THUMBNAIL_PATH.to_owned(),
            filename: "thumbnail".to_owned(),
            kind,
            ueberzug: ueberzug::Ueberzug::new(),
            length: 0,
            index: 0,
        }
    }

    fn make(filepath: &Path, kind: UeberzugKind) -> Result<Self> {
        match kind {
            UeberzugKind::Font => Self::font_thumbnail(filepath),
            UeberzugKind::Image => Self::image_thumbnail(filepath),
            UeberzugKind::Office => Self::office_thumbnail(filepath),
            UeberzugKind::Pdf => Self::pdf_thumbnail(filepath),
            UeberzugKind::Svg => Self::svg_thumbnail(filepath),
            UeberzugKind::Video => Self::video_thumbnail(filepath),
        }
    }

    fn image_thumbnail(img_path: &Path) -> Result<Self> {
        let filename = filename_from_path(img_path)?.to_owned();
        let path = img_path
            .to_str()
            .context("ueberzug: couldn't parse the path into a string")?
            .to_owned();
        Ok(Self {
            original: img_path.to_owned(),
            path,
            filename,
            kind: UeberzugKind::Image,
            ueberzug: ueberzug::Ueberzug::new(),
            length: 0,
            index: 0,
        })
    }

    fn video_thumbnail(video_path: &Path) -> Result<Self> {
        Self::make_video_thumbnail(video_path)?;
        Ok(Self::thumbnail(video_path.to_owned(), UeberzugKind::Video))
    }

    fn font_thumbnail(font_path: &Path) -> Result<Self> {
        Self::make_font_thumbnail(font_path)?;
        Ok(Self::thumbnail(font_path.to_owned(), UeberzugKind::Font))
    }

    fn svg_thumbnail(svg_path: &Path) -> Result<Self> {
        Self::make_svg_thumbnail(svg_path)?;
        Ok(Self::thumbnail(svg_path.to_owned(), UeberzugKind::Svg))
    }

    fn office_thumbnail(calc_path: &Path) -> Result<Self> {
        let calc_str = path_to_string(&calc_path);
        let args = vec!["--convert-to", "pdf", "--outdir", "/tmp", &calc_str];
        let output = std::process::Command::new(LIBREOFFICE)
            .args(args)
            .output()?;
        if !output.stderr.is_empty() {
            info!(
                "libreoffice conversion output: {} {}",
                String::from_utf8(output.stdout).unwrap_or_default(),
                String::from_utf8(output.stderr).unwrap_or_default()
            );
            return Err(anyhow!("{LIBREOFFICE} couldn't convert {calc_str} to pdf"));
        }
        let mut pdf_path = std::path::PathBuf::from("/tmp");
        let filename = calc_path.file_name().context("")?;
        pdf_path.push(filename);
        pdf_path.set_extension("pdf");
        std::fs::rename(pdf_path, CALC_PDF_PATH)?;
        let calc_pdf_path = PathBuf::from(CALC_PDF_PATH);
        let length = Self::make_pdf_thumbnail(&calc_pdf_path, 0)?;
        let mut thumbnail = Self::thumbnail(calc_pdf_path.to_owned(), UeberzugKind::Pdf);
        thumbnail.length = length;
        Ok(thumbnail)
    }

    fn pdf_thumbnail(pdf_path: &Path) -> Result<Self> {
        let length = Self::make_pdf_thumbnail(pdf_path, 0)?;
        let mut thumbnail = Self::thumbnail(pdf_path.to_owned(), UeberzugKind::Pdf);
        thumbnail.length = length;
        Ok(thumbnail)
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

    /// Creates the thumbnail for the `index` page.
    /// Returns the number of pages of the pdf.
    ///
    /// It may fail (and surelly crash the app) if the pdf is password protected.
    /// We pass a generic password which is hardcoded.
    fn make_pdf_thumbnail(pdf_path: &Path, index: usize) -> Result<usize> {
        let doc: poppler::PopplerDocument =
            poppler::PopplerDocument::new_from_file(pdf_path, "upw")?;
        let length = doc.get_n_pages();
        if index >= length {
            return Err(anyhow!(
                "Poppler: index {index} >= number of page {length}."
            ));
        }
        let page: poppler::PopplerPage = doc
            .get_page(index)
            .context("poppler couldn't extract page")?;
        let (w, h) = page.get_size();
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32)?;
        let ctx = cairo::Context::new(&surface)?;
        ctx.save()?;
        page.render(&ctx);
        ctx.restore()?;
        ctx.show_page()?;
        let mut file = std::fs::File::create(THUMBNAIL_PATH)?;
        surface.write_to_png(&mut file)?;
        Ok(length)
    }

    fn make_video_thumbnail(video_path: &Path) -> Result<()> {
        let path_str = video_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make_thumbnail(
            FFMPEG,
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
        Self::make_thumbnail(FONTIMAGE, &["-o", THUMBNAIL_PATH, path_str])
    }

    fn make_svg_thumbnail(svg_path: &Path) -> Result<()> {
        let path_str = svg_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make_thumbnail(
            RSVG_CONVERT,
            &["--keep-aspect-ratio", path_str, "-o", THUMBNAIL_PATH],
        )
    }

    /// Only affect pdf thumbnail. Will decrease the index if possible.
    pub fn up_one_row(&mut self) {
        if let UeberzugKind::Pdf = self.kind {
            if self.index > 0 {
                self.index -= 1;
            }
        }
    }

    /// Only affect pdf thumbnail. Will increase the index if possible.
    pub fn down_one_row(&mut self) {
        if let UeberzugKind::Pdf = self.kind {
            if self.index + 1 < self.len() {
                self.index += 1;
            }
        }
    }

    /// 0 for every kind except pdf where it's the number of pages.
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Update the thumbnail of the pdf to match its own index.
    /// Does nothing for other kinds.
    pub fn match_index(&self) -> Result<()> {
        if let UeberzugKind::Pdf = self.kind {
            Self::make_pdf_thumbnail(&self.original, self.index)?;
        }
        Ok(())
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
}

impl Directory {
    pub fn new(path: PathBuf, users: &Users) -> Self {
        let tree = Tree::new(
            path,
            4,
            SortKind::tree_default(),
            users,
            false,
            &FilterKind::All,
        );

        let (_selected_index, content) = tree.into_navigable_content(users);

        Self { content }
    }

    pub fn empty() -> Self {
        Self { content: vec![] }
    }

    pub fn len(&self) -> usize {
        self.content.len()
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
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
pub type ColoredTriplet = (String, String, ColoredString);

pub trait MakeTriplet {
    fn make(
        fileinfo: &FileInfo,
        prefix: &str,
        filename_text: String,
        color_effect: ColorEffect,
        current_path: &std::path::Path,
    ) -> ColoredTriplet;
}

impl MakeTriplet for ColoredTriplet {
    #[inline]
    fn make(
        fileinfo: &FileInfo,
        prefix: &str,
        filename_text: String,
        color_effect: ColorEffect,
        current_path: &std::path::Path,
    ) -> ColoredTriplet {
        (
            fileinfo.format_no_filename().unwrap_or_default(),
            prefix.to_owned(),
            ColoredString::new(filename_text, color_effect, current_path.to_owned()),
        )
    }
}
/// A vector of highlighted strings
pub type VecSyntaxedString = Vec<SyntaxedString>;

impl_window!(HLContent, VecSyntaxedString);
impl_window!(TextContent, String);
impl_window!(BinaryContent, Line);
impl_window!(ArchiveContent, String);
impl_window!(MediaContent, String);
impl_window!(Directory, ColoredTriplet);
impl_window!(Diff, String);
impl_window!(Iso, String);
impl_window!(ColoredText, String);
impl_window!(Socket, String);
impl_window!(BlockDevice, String);
impl_window!(FifoCharDevice, String);
