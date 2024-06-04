use std::cmp::min;
use std::fmt::Write as _;
use std::fs::metadata;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Cursor;
use std::io::{BufRead, BufReader, Read};
use std::iter::{Enumerate, Skip, Take};
use std::path::{Path, PathBuf};
use std::slice::Iter;

use anyhow::{anyhow, Context, Result};
use content_inspector::{inspect, ContentType};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tuikit::attr::{Attr, Color};

use crate::common::{clear_tmp_file, filename_from_path, is_program_in_path, path_to_string};
use crate::common::{
    FFMPEG, FONTIMAGE, ISOINFO, JUPYTER, LIBREOFFICE, LSBLK, LSOF, MEDIAINFO, PANDOC, PDFINFO,
    PDFTOPPM, RSVG_CONVERT, SS, TRANSMISSION_SHOW, UEBERZUG,
};
use crate::io::{
    execute_and_capture_output, execute_and_capture_output_without_check, execute_and_output_no_log,
};
use crate::log_info;
use crate::modes::{
    extract_extension, list_files_tar, list_files_zip, FileKind, FilterKind, Scalers, SortKind,
    Tree, TreeLineBuilder, TreeLines, UeConf, Ueberzug, Users,
};

/// Different kind of extension for grouped by previewers.
/// Any extension we can preview should be matched here.
#[derive(Default, Eq, PartialEq)]
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
    Torrent,
    #[default]
    Default,
}

impl ExtensionKind {
    /// Match any known extension against an extension kind.
    pub fn matcher(ext: &str) -> Self {
        match ext {
            "zip" | "gzip" | "bzip2" | "xz" | "lzip" | "lzma" | "tar" | "mtree" | "raw" | "7z"
            | "gz" | "zst" | "deb" | "rpm" => Self::Archive,
            "png" | "jpg" | "jpeg" | "tiff" | "heif" | "gif" | "cr2" | "nef" | "orf" | "sr2" => {
                Self::Image
            }
            "ogg" | "ogm" | "riff" | "mp2" | "mp3" | "wm" | "qt" | "ac3" | "dts" | "aac"
            | "mac" | "flac" => Self::Audio,
            "mkv" | "webm" | "mpeg" | "mp4" | "avi" | "flv" | "mpg" | "wmv" | "m4v" | "mov" => {
                Self::Video
            }
            "ttf" | "otf" => Self::Font,
            "svg" | "svgz" => Self::Svg,
            "pdf" => Self::Pdf,
            "iso" => Self::Iso,
            "ipynb" => Self::Notebook,
            "doc" | "docx" | "odt" | "sxw" | "xlsx" | "xls" => Self::Office,
            "epub" => Self::Epub,
            "torrent" => Self::Torrent,
            _ => Self::Default,
        }
    }

    fn has_programs(&self) -> bool {
        match self {
            Self::Pdf => {
                is_program_in_path(UEBERZUG)
                    && is_program_in_path(PDFINFO)
                    && is_program_in_path(PDFTOPPM)
            }
            Self::Image => is_program_in_path(UEBERZUG),
            Self::Audio => is_program_in_path(MEDIAINFO),
            Self::Video => is_program_in_path(UEBERZUG) && is_program_in_path(FFMPEG),
            Self::Font => is_program_in_path(UEBERZUG) && is_program_in_path(FONTIMAGE),
            Self::Svg => is_program_in_path(UEBERZUG) && is_program_in_path(RSVG_CONVERT),
            Self::Iso => is_program_in_path(ISOINFO),
            Self::Notebook => is_program_in_path(JUPYTER),
            Self::Office => is_program_in_path(LIBREOFFICE),
            Self::Epub => is_program_in_path(PANDOC),
            Self::Torrent => is_program_in_path(TRANSMISSION_SHOW),

            _ => true,
        }
    }
}

impl std::fmt::Display for ExtensionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Archive => write!(f, "archive"),
            Self::Image => write!(f, "image"),
            Self::Audio => write!(f, "audio"),
            Self::Video => write!(f, "video"),
            Self::Font => write!(f, "font"),
            Self::Svg => write!(f, "svg"),
            Self::Pdf => write!(f, "pdf"),
            Self::Iso => write!(f, "iso"),
            Self::Notebook => write!(f, "notebook"),
            Self::Office => write!(f, "office"),
            Self::Epub => write!(f, "epub"),
            Self::Torrent => write!(f, "torrent"),
            Self::Default => write!(f, "default"),
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
    Ueberzug(UeberzugPreview),
    Media(MediaContent),
    Tree(TreePreview),
    Iso(Iso),
    ColoredText(ColoredText),
    Socket(Socket),
    BlockDevice(BlockDevice),
    FifoCharDevice(FifoCharDevice),
    Torrent(Torrent),
    #[default]
    Empty,
}

impl Preview {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    pub fn kind(&self) -> &str {
        match self {
            Self::Syntaxed(_) => "Syntaxed",
            Self::Text(_) => "Text",
            Self::Binary(_) => "Binary",
            Self::Archive(_) => "Archive",
            Self::Ueberzug(_) => "Ueberzug",
            Self::Media(_) => "Media",
            Self::Tree(_) => "Tree",
            Self::Iso(_) => "Iso",
            Self::ColoredText(_) => "ColoredText",
            Self::Socket(_) => "Socket",
            Self::BlockDevice(_) => "BlockDevice",
            Self::FifoCharDevice(_) => "FifoCharDevice",
            Self::Torrent(_) => "Torrent",
            Self::Empty => "Empty",
        }
    }

    /// Empty preview, holding nothing.
    pub fn empty() -> Self {
        clear_tmp_file();
        Self::Empty
    }

    pub fn hide(&self, ueberzug: &Ueberzug) {
        if let Self::Ueberzug(ueb) = self {
            ueb.hide(ueberzug)
        }
    }

    pub fn new<P>(path: P, users: &Users) -> Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        let file_kind = FileKind::new(&metadata(path)?, path);
        match file_kind {
            FileKind::Directory => Self::directory(path, users),
            _ => Self::file(file_kind, path),
        }
    }

    /// Creates a new `Directory` from the file_info
    /// It explores recursivelly the directory and creates a tree.
    /// The recursive exploration is limited to depth 2.
    pub fn directory(path: &std::path::Path, users: &Users) -> Result<Self> {
        Ok(Self::Tree(TreePreview::new(
            std::sync::Arc::from(path),
            users,
        )))
    }

    /// Creates a new preview instance based on the filekind and the extension of
    /// the file.
    /// Sometimes it reads the content of the file, sometimes it delegates
    /// it to the display method.
    /// Directories aren't handled there since we need more arguments to create
    /// their previews.
    pub fn file(file_kind: FileKind<bool>, path: &std::path::Path) -> Result<Self> {
        clear_tmp_file();
        match file_kind {
            FileKind::Directory => Err(anyhow!("{path} is a directory", path = path.display())),
            FileKind::NormalFile => Self::normal_file(path),
            FileKind::Socket if is_program_in_path(SS) => Ok(Self::socket(path)),
            FileKind::BlockDevice if is_program_in_path(LSBLK) => Ok(Self::blockdevice(path)),
            FileKind::Fifo | FileKind::CharDevice if is_program_in_path(LSOF) => {
                Ok(Self::fifo_chardevice(path))
            }
            _ => Ok(Preview::default()),
        }
    }

    fn normal_file(path: &std::path::Path) -> Result<Self> {
        let extension = &extract_extension(path).to_lowercase();
        let kind = ExtensionKind::matcher(extension);
        match kind {
            ExtensionKind::Archive => Ok(Self::Archive(ArchiveContent::new(path, extension)?)),
            ExtensionKind::Pdf if kind.has_programs() => Ok(Self::Ueberzug(UeberzugPreview::new(
                path,
                UeberzugKind::Pdf,
            ))),
            ExtensionKind::Image if kind.has_programs() => Ok(Self::Ueberzug(
                UeberzugPreview::new(path, UeberzugKind::Image),
            )),
            ExtensionKind::Audio if kind.has_programs() => {
                Ok(Self::Media(MediaContent::new(path)?))
            }
            ExtensionKind::Video if kind.has_programs() => Ok(Self::Ueberzug(
                UeberzugPreview::new(path, UeberzugKind::Video),
            )),
            ExtensionKind::Font if kind.has_programs() => Ok(Self::Ueberzug(UeberzugPreview::new(
                path,
                UeberzugKind::Font,
            ))),
            ExtensionKind::Svg if kind.has_programs() => Ok(Self::Ueberzug(UeberzugPreview::new(
                path,
                UeberzugKind::Svg,
            ))),
            ExtensionKind::Iso if kind.has_programs() => Ok(Self::Iso(Iso::new(path)?)),
            ExtensionKind::Notebook if kind.has_programs() => {
                Ok(Self::notebook(path).context("Preview: Couldn't parse notebook")?)
            }
            ExtensionKind::Office if kind.has_programs() => Ok(Self::Ueberzug(
                UeberzugPreview::new(path, UeberzugKind::Office),
            )),
            ExtensionKind::Epub if kind.has_programs() => {
                Ok(Self::epub(path).context("Preview: Couldn't parse epub")?)
            }
            ExtensionKind::Torrent if kind.has_programs() => {
                Ok(Self::torrent(path).context("Preview couldn't explore the torrent file")?)
            }
            _ => match Self::preview_syntaxed(extension, path) {
                Some(syntaxed_preview) => Ok(syntaxed_preview),
                None => Self::preview_text_or_binary(path),
            },
        }
    }

    fn socket(path: &std::path::Path) -> Self {
        Self::Socket(Socket::new(path))
    }

    fn blockdevice(path: &std::path::Path) -> Self {
        Self::BlockDevice(BlockDevice::new(path))
    }

    fn fifo_chardevice(path: &std::path::Path) -> Self {
        Self::FifoCharDevice(FifoCharDevice::new(path))
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

    fn preview_text_or_binary(path: &std::path::Path) -> Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let mut buffer = vec![0; Self::CONTENT_INSPECTOR_MIN_SIZE];
        if Self::is_binary(path, &mut file, &mut buffer) {
            Ok(Self::Binary(BinaryContent::new(path)?))
        } else {
            Ok(Self::Text(TextContent::from_file(path)?))
        }
    }

    fn is_binary(path: &std::path::Path, file: &mut std::fs::File, buffer: &mut [u8]) -> bool {
        let Ok(meta) = metadata(path) else {
            return false;
        };
        meta.len() >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
            && file.read_exact(buffer).is_ok()
            && inspect(buffer) == ContentType::BINARY
    }

    /// Creates the help preview as if it was a text file.
    pub fn help(help: &str) -> Self {
        Self::Text(TextContent::help(help))
    }

    pub fn log(log: Vec<String>) -> Self {
        Self::Text(TextContent::log(log))
    }

    pub fn cli_info(output: &str, command: String) -> Self {
        Self::ColoredText(ColoredText::new(output, command))
    }

    pub fn epub(path: &Path) -> Result<Self> {
        Ok(Self::Text(
            TextContent::epub(path).context("Couldn't read epub")?,
        ))
    }

    pub fn torrent(path: &Path) -> Result<Self> {
        Ok(Self::Torrent(Torrent::new(path).context("")?))
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
            Self::Tree(tree) => tree.len(),
            Self::Iso(iso) => iso.len(),
            Self::ColoredText(text) => text.len(),
            Self::Socket(socket) => socket.len(),
            Self::BlockDevice(blockdevice) => blockdevice.len(),
            Self::FifoCharDevice(fifo) => fifo.len(),
            Self::Torrent(torrent) => torrent.len(),
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
    fn new(path: &Path) -> Self {
        let filename = filename_from_path(path).unwrap_or_default();
        let content: Vec<String>;
        if let Ok(output) = execute_and_output_no_log(SS, ["-lpmepiT"]) {
            let s = String::from_utf8(output.stdout).unwrap_or_default();
            content = s
                .lines()
                .filter(|l| l.contains(filename))
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
    fn new(path: &Path) -> Self {
        let content: Vec<String>;
        if let Ok(output) = execute_and_output_no_log(
            LSBLK,
            [
                "-lfo",
                "FSTYPE,PATH,LABEL,UUID,FSVER,MOUNTPOINT,MODEL,SIZE,FSAVAIL,FSUSE%",
                &path_to_string(&path),
            ],
        ) {
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
    fn new(path: &std::path::Path) -> Self {
        let content: Vec<String>;
        if let Ok(output) = execute_and_output_no_log(LSOF, [path_to_string(&path).as_str()]) {
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
            "../../../assets/themes/Monokai_Extended.tmTheme"
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

    fn new(path: &std::path::Path) -> Result<Self> {
        let mut reader = BufReader::new(std::fs::File::open(path)?);
        let mut buffer = [0; Self::LINE_WIDTH];
        let mut content: Vec<Line> = vec![];
        while let Ok(nb_bytes_read) = reader.read(&mut buffer[..]) {
            if nb_bytes_read != Self::LINE_WIDTH {
                content.push(Line::new((&buffer[0..nb_bytes_read]).into()));
                break;
            }
            content.push(Line::new(buffer.into()));
            if content.len() >= Self::SIZE_LIMIT {
                break;
            }
        }

        let meta = metadata(path)?;
        let true_size = meta.len();
        Ok(Self {
            path: path.to_path_buf(),
            length: true_size / Self::LINE_WIDTH as u64,
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
            "zst" | "gz" | "bz" | "xz" | "gzip" | "bzip2" | "deb" | "rpm" => {
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
        if let Ok(output) = execute_and_output_no_log(MEDIAINFO, [path_to_string(&path).as_str()]) {
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

#[derive(Clone)]
pub enum UeberzugKind {
    Font,
    Image,
    Office,
    Pdf,
    Svg,
    Video,
}

impl UeberzugKind {
    fn ext(&self) -> &'static str {
        match self {
            Self::Office => "jpg",
            Self::Pdf => "jpg",
            Self::Image => "jpg",
            Self::Svg => "png",
            Self::Font => "png",
            Self::Video => "jpg",
        }
    }
}

#[derive(Clone)]
pub struct UeberzugPreview {
    kind: UeberzugKind,
    original: PathBuf,
    identifier: PathBuf,
    length: usize,
    pub index: usize,
}

impl UeberzugPreview {
    pub fn new(original: &Path, kind: UeberzugKind) -> Self {
        let identifier = Self::create_identifier(original, &kind);
        let length = Self::calc_length(original, &kind);
        let original = original.to_owned();
        let index = 0;
        Self {
            kind,
            original,
            identifier,
            length,
            index,
        }
    }

    pub fn update_office_len(&mut self) {
        self.length = Self::calc_length(&self.original, &self.kind);
        log_info!(
            "update_office_len: {path}: len {len}",
            path = self.original.display(),
            len = self.length
        );
    }

    pub fn has_multiple_pages(&self) -> bool {
        matches!(self.kind, UeberzugKind::Pdf | UeberzugKind::Office)
    }

    fn get_pdf_length(path: &Path) -> Result<usize> {
        let output =
            execute_and_capture_output(PDFINFO, &[path.to_string_lossy().to_string().as_ref()])?;
        let line = output.lines().find(|line| line.starts_with("Pages: "));

        match line {
            Some(line) => {
                let page_count_str = line.split_whitespace().nth(1).unwrap();
                let page_count = page_count_str.parse::<usize>()?;
                Ok(page_count)
            }
            None => Err(anyhow::Error::msg("Couldn't find the page number")),
        }
    }

    fn calc_length(original_path: &Path, kind: &UeberzugKind) -> usize {
        match &kind {
            UeberzugKind::Pdf => Self::get_pdf_length(original_path).unwrap_or(1),
            UeberzugKind::Office => {
                Self::get_pdf_length(&Thumbnail::office_pdf_name(original_path)).unwrap_or(1)
            }
            _ => 1,
        }
    }

    fn create_identifier(original_path: &Path, kind: &UeberzugKind) -> PathBuf {
        // Ensure images are displayed with their correct path
        if matches!(kind, UeberzugKind::Image) {
            return original_path.to_owned();
        }
        let ext = UeberzugKind::ext(kind);
        let mut s = DefaultHasher::new();
        original_path.hash(&mut s);
        let hashed_path = s.finish();
        PathBuf::from(format!("/tmp/fm_thumbnail_{hashed_path}.{ext}"))
    }

    fn len(&self) -> usize {
        self.length
    }

    pub fn build_thumbnail(&self) -> Result<()> {
        if self.identifier.exists() {
            return Ok(());
        }
        match self.kind {
            UeberzugKind::Image => (),
            UeberzugKind::Video => Thumbnail::video(&self.original, &self.identifier)?,
            UeberzugKind::Font => Thumbnail::font(&self.original, &self.identifier)?,
            UeberzugKind::Svg => Thumbnail::svg(&self.original, &self.identifier)?,
            UeberzugKind::Pdf => Thumbnail::pdf(&self.original, self.index, &self.identifier)?,
            UeberzugKind::Office => {
                Thumbnail::office(&self.original, self.index, &self.identifier)?
            }
        };
        Ok(())
    }

    pub fn draw(&self, ueberzug: &Ueberzug, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
        self.build_thumbnail()?;
        let identifier = self.identifier.to_string_lossy();
        let ueconf = Self::build_ue_conf(identifier.as_ref(), x, y, width, height);
        ueberzug.draw(&ueconf);
        Ok(())
    }

    fn build_ue_conf(identifier: &str, x: u16, y: u16, width: u16, height: u16) -> UeConf {
        UeConf {
            identifier,
            path: identifier,
            x,
            y,
            width: Some(width),
            height: Some(height),
            scaler: Some(Scalers::FitContain),
            ..Default::default()
        }
    }

    fn delete_identifier(&self) -> bool {
        if self.identifier.exists() {
            std::fs::remove_file(&self.identifier).is_ok()
        } else {
            false
        }
    }

    pub fn up_one_row(&mut self) {
        if matches!(self.kind, UeberzugKind::Pdf | UeberzugKind::Office) && self.index > 0 {
            self.index -= 1;
            // this is required since the pdf file is already displayed which means the file is created and no thumbnail will be made
            self.delete_identifier();
        }
    }

    pub fn down_one_row(&mut self) {
        if matches!(self.kind, UeberzugKind::Office) {
            log_info!("down_one_row: office");
            self.update_office_len();
        }
        if matches!(self.kind, UeberzugKind::Pdf | UeberzugKind::Office)
            && self.index + 1 < self.length
        {
            self.index += 1;
            // this is required since the pdf file is already displayed which means the file is created and no thumbnail will be made
            self.delete_identifier();
        }
    }

    pub fn hide(&self, ueberzug: &Ueberzug) {
        ueberzug.clear(self.identifier.to_string_lossy().as_ref())
    }
}

struct Thumbnail;

impl Thumbnail {
    fn make(exe: &str, args: &[&str]) -> Result<()> {
        let output = execute_and_output_no_log(exe, args.to_owned())?;
        if !output.stderr.is_empty() {
            log_info!(
                "make thumbnail output: {} {}",
                String::from_utf8(output.stdout).unwrap_or_default(),
                String::from_utf8(output.stderr).unwrap_or_default()
            );
        }
        Ok(())
    }

    fn video(video_path: &Path, output_path: &Path) -> Result<()> {
        let path_str = video_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make(
            FFMPEG,
            &[
                "-i",
                path_str,
                "-vf",
                "thumbnail",
                "-frames:v",
                "1",
                output_path.to_string_lossy().as_ref(),
                "-y",
            ],
        )
    }

    fn svg(svg_path: &Path, output_path: &Path) -> Result<()> {
        let path_str = svg_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make(
            RSVG_CONVERT,
            &[
                "--keep-aspect-ratio",
                path_str,
                "-o",
                output_path.to_string_lossy().as_ref(),
            ],
        )
    }

    fn font(font_path: &Path, output_path: &Path) -> Result<()> {
        let path_str = font_path
            .to_str()
            .context("make_thumbnail: couldn't parse the path into a string")?;
        Self::make(
            FONTIMAGE,
            &["-o", output_path.to_string_lossy().as_ref(), path_str],
        )
    }

    fn pdf(path: &Path, page_number: usize, output_path: &Path) -> Result<()> {
        let output_path = output_path.to_string_lossy().to_string().to_owned();
        let Some(output_path) = output_path.strip_suffix(".jpg") else {
            return Err(anyhow!(
                "output_path should end with .jpg. Got {output_path}"
            ));
        };
        execute_and_capture_output_without_check(
            PDFTOPPM,
            &[
                "-singlefile",
                "-jpeg",
                "-jpegopt",
                "quality=75",
                "-f",
                (page_number + 1).to_string().as_ref(),
                path.to_string_lossy().as_ref(),
                &output_path,
            ],
        )?;
        Ok(())
    }

    fn office(calc_path: &Path, page_index: usize, output_path: &Path) -> Result<()> {
        let calc_str = path_to_string(&calc_path);
        let args = ["--convert-to", "pdf", "--outdir", "/tmp", &calc_str];
        let output = execute_and_output_no_log(LIBREOFFICE, args)?;
        if !output.stderr.is_empty() {
            log_info!(
                "libreoffice conversion output: {} {}",
                String::from_utf8(output.stdout).unwrap_or_default(),
                String::from_utf8(output.stderr).unwrap_or_default()
            );
            return Err(anyhow!("{LIBREOFFICE} couldn't convert {calc_str} to pdf"));
        }
        let pdf_path = Self::rename_office_pdf(calc_path)?;
        Self::pdf(&pdf_path, page_index, output_path)
    }

    fn office_built_pdf_name(calc_path: &Path) -> PathBuf {
        let mut built_pdf_path = std::path::PathBuf::from("/tmp");
        let filename = calc_path.file_name().unwrap_or_default();
        built_pdf_path.push(filename);
        built_pdf_path.set_extension("pdf");
        built_pdf_path
    }

    fn office_pdf_name(calc_path: &Path) -> PathBuf {
        let mut pdf_path = std::path::PathBuf::from("/tmp");
        let filename = calc_path.file_name().unwrap_or_default();
        pdf_path.push(filename);
        pdf_path.set_extension("pdf");
        let new_filename = format!(
            "fm_thumbnail_{filename}",
            filename = filename.to_string_lossy()
        );
        let mut new_pdf_path = std::path::PathBuf::from("/tmp");
        new_pdf_path.push(new_filename);
        new_pdf_path
    }

    fn rename_office_pdf(calc_path: &Path) -> Result<PathBuf> {
        let built_pdf_path = Self::office_built_pdf_name(calc_path);
        let new_pdf_path = Self::office_pdf_name(calc_path);
        std::fs::rename(&built_pdf_path, &new_pdf_path)?;
        Ok(new_pdf_path)
    }
}

#[derive(Clone, Debug)]
pub struct ColoredText {
    title: String,
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

    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    /// Make a new previewed colored text.
    pub fn new(output: &str, title: String) -> Self {
        let content: Vec<String> = output.lines().map(|line| line.to_owned()).collect();
        let len = content.len();
        let selected_index = 0;
        Self {
            title,
            content,
            len,
            selected_index,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TreePreview {
    pub tree: Tree,
}

impl TreePreview {
    pub fn new(path: std::sync::Arc<Path>, users: &Users) -> Self {
        let tree = Tree::new(
            path,
            4,
            SortKind::tree_default(),
            users,
            false,
            &FilterKind::All,
        );
        Self { tree }
    }

    pub fn len(&self) -> usize {
        self.tree.displayable().lines().len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
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

        Ok(Self {
            length: content.len(),
            content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }
}

pub struct Torrent {
    pub content: Vec<String>,
    length: usize,
}

impl Torrent {
    fn new(path: &Path) -> Result<Self> {
        let path = path.to_str().context("couldn't parse the path")?;
        let content: Vec<String> =
            execute_and_capture_output_without_check(TRANSMISSION_SHOW, &[path])?
                .lines()
                .map(|s| s.to_owned())
                .collect();
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

/// A vector of highlighted strings
pub type VecSyntaxedString = Vec<SyntaxedString>;

impl_window!(HLContent, VecSyntaxedString);
impl_window!(TextContent, String);
impl_window!(BinaryContent, Line);
impl_window!(ArchiveContent, String);
impl_window!(MediaContent, String);
impl_window!(Iso, String);
impl_window!(ColoredText, String);
impl_window!(Socket, String);
impl_window!(BlockDevice, String);
impl_window!(FifoCharDevice, String);
impl_window!(TreeLines, TreeLineBuilder);
impl_window!(Torrent, String);
