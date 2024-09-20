use std::cmp::min;
use std::convert::Into;
use std::fmt::Write as _;
use std::fs::metadata;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::iter::{Enumerate, Skip, Take};
use std::path::{Path, PathBuf};
use std::slice::Iter;

use anyhow::{anyhow, Context, Result};
use content_inspector::{inspect, ContentType};
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};
use tuikit::attr::{Attr, Color, Effect};

use crate::common::{
    clear_tmp_files, is_program_in_path, path_to_string, FFMPEG, FONTIMAGE, ISOINFO, JUPYTER,
    LIBREOFFICE, LSBLK, LSOF, MEDIAINFO, PANDOC, PDFINFO, PDFTOPPM, RSVG_CONVERT, SS,
    TRANSMISSION_SHOW, UEBERZUG,
};
use crate::config::MONOKAI_THEME;
use crate::io::{execute_and_capture_output_without_check, execute_and_output_no_log};
use crate::modes::{
    list_files_tar, list_files_zip, ContentWindow, FileInfo, FileKind, FilterKind, SortKind, Tree,
    TreeLineBuilder, TreeLines, Ueber, UeberBuilder, Users,
};

/// Different kind of extension for grouped by previewers.
/// Any extension we can preview should be matched here.
#[derive(Default, Eq, PartialEq)]
pub enum ExtensionKind {
    Archive,
    Audio,
    Epub,
    Font,
    Image,
    Iso,
    Notebook,
    Office,
    Pdf,
    Svg,
    Torrent,
    Video,

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
            Self::Audio => is_program_in_path(MEDIAINFO),
            Self::Epub => is_program_in_path(PANDOC),
            Self::Font => is_program_in_path(UEBERZUG) && is_program_in_path(FONTIMAGE),
            Self::Image => is_program_in_path(UEBERZUG),
            Self::Iso => is_program_in_path(ISOINFO),
            Self::Notebook => is_program_in_path(JUPYTER),
            Self::Office => is_program_in_path(LIBREOFFICE),
            Self::Pdf => {
                is_program_in_path(UEBERZUG)
                    && is_program_in_path(PDFINFO)
                    && is_program_in_path(PDFTOPPM)
            }
            Self::Svg => is_program_in_path(UEBERZUG) && is_program_in_path(RSVG_CONVERT),
            Self::Torrent => is_program_in_path(TRANSMISSION_SHOW),
            Self::Video => is_program_in_path(UEBERZUG) && is_program_in_path(FFMPEG),

            _ => true,
        }
    }

    fn is_ueber_kind(&self) -> bool {
        matches!(
            &self,
            ExtensionKind::Font
                | ExtensionKind::Image
                | ExtensionKind::Office
                | ExtensionKind::Pdf
                | ExtensionKind::Svg
                | ExtensionKind::Video
        )
    }
}

impl std::fmt::Display for ExtensionKind {
    #[rustfmt::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Archive   => write!(f, "archive"),
            Self::Image     => write!(f, "image"),
            Self::Audio     => write!(f, "audio"),
            Self::Video     => write!(f, "video"),
            Self::Font      => write!(f, "font"),
            Self::Svg       => write!(f, "svg"),
            Self::Pdf       => write!(f, "pdf"),
            Self::Iso       => write!(f, "iso"),
            Self::Notebook  => write!(f, "notebook"),
            Self::Office    => write!(f, "office"),
            Self::Epub      => write!(f, "epub"),
            Self::Torrent   => write!(f, "torrent"),
            Self::Default   => write!(f, "default"),
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
    Ueberzug(Ueber),
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
        matches!(self, Self::Empty)
    }

    /// Creates a new, static window used when we display a preview in the second pane
    pub fn window_for_second_pane(&self, height: usize) -> ContentWindow {
        ContentWindow::new(self.len(), height)
    }
}

pub struct PreviewBuilder<'a> {
    file_info: &'a FileInfo,
    users: &'a Users,
}

impl<'a> PreviewBuilder<'a> {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    pub fn new(file_info: &'a FileInfo, users: &'a Users) -> Self {
        Self { file_info, users }
    }

    /// Empty preview, holding nothing.
    pub fn empty() -> Preview {
        clear_tmp_files();
        Preview::Empty
    }

    pub fn build(&self) -> Result<Preview> {
        match self.file_info.file_kind {
            FileKind::Directory => self.directory(),
            _ => self.file(),
        }
    }

    /// Creates a new `Directory` from the self.file_info
    /// It explores recursivelly the directory and creates a tree.
    /// The recursive exploration is limited to depth 2.
    fn directory(&self) -> Result<Preview> {
        Ok(Preview::Tree(TreePreview::new(
            self.file_info.path.clone(),
            self.users,
        )))
    }

    /// Creates a new preview instance based on the filekind and the extension of
    /// the file.
    /// Sometimes it reads the content of the file, sometimes it delegates
    /// it to the display method.
    /// Directories aren't handled there since we need more arguments to create
    /// their previews.
    fn file(&self) -> Result<Preview> {
        clear_tmp_files();
        match self.file_info.file_kind {
            FileKind::Directory => Err(anyhow!(
                "{p} is a directory",
                p = self.file_info.path.display()
            )),
            FileKind::NormalFile => self.normal_file(),
            FileKind::Socket if is_program_in_path(SS) => Ok(self.socket()),
            FileKind::BlockDevice if is_program_in_path(LSBLK) => Ok(self.blockdevice()),
            FileKind::Fifo | FileKind::CharDevice if is_program_in_path(LSOF) => {
                Ok(self.fifo_chardevice())
            }
            _ => Ok(Preview::default()),
        }
    }

    fn normal_file(&self) -> Result<Preview> {
        let extension = &self.file_info.extension.to_lowercase();
        let path = &self.file_info.path;
        let kind = ExtensionKind::matcher(extension);
        match kind {
            ExtensionKind::Archive => Ok(Preview::Archive(ArchiveContent::new(
                &self.file_info.path,
                extension,
            )?)),
            ExtensionKind::Iso if kind.has_programs() => Ok(Preview::Iso(Iso::new(path)?)),
            ExtensionKind::Epub if kind.has_programs() => Ok(Preview::Text(
                TextContent::epub(path).context("Preview: Couldn't read epub")?,
            )),
            ExtensionKind::Torrent if kind.has_programs() => Ok(Preview::Torrent(
                Torrent::new(path).context("Preview: Couldn't read torrent")?,
            )),
            ExtensionKind::Notebook if kind.has_programs() => {
                Ok(Self::notebook(path).context("Preview: Couldn't parse notebook")?)
            }
            ExtensionKind::Audio if kind.has_programs() => {
                Ok(Preview::Media(MediaContent::new(path)?))
            }
            _ if kind.is_ueber_kind() && kind.has_programs() => Self::ueber(path, kind),
            _ => match self.syntaxed(extension) {
                Some(syntaxed_preview) => Ok(syntaxed_preview),
                None => self.text_or_binary(),
            },
        }
    }

    fn ueber(path: &Path, kind: ExtensionKind) -> Result<Preview> {
        let preview = UeberBuilder::new(path, kind.into()).build()?;
        if preview.is_empty() {
            Ok(Preview::Empty)
        } else {
            Ok(Preview::Ueberzug(preview))
        }
    }

    fn socket(&self) -> Preview {
        Preview::Socket(Socket::new(self.file_info))
    }

    fn blockdevice(&self) -> Preview {
        Preview::BlockDevice(BlockDevice::new(self.file_info))
    }

    fn fifo_chardevice(&self) -> Preview {
        Preview::FifoCharDevice(FifoCharDevice::new(self.file_info))
    }

    fn syntaxed(&self, ext: &str) -> Option<Preview> {
        if let Ok(metadata) = metadata(&self.file_info.path) {
            if metadata.len() > HLContent::SIZE_LIMIT as u64 {
                return None;
            }
        } else {
            return None;
        };
        let ss = SyntaxSet::load_defaults_nonewlines();
        ss.find_syntax_by_extension(ext).map(|syntax| {
            Preview::Syntaxed(
                HLContent::new(&self.file_info.path, ss.clone(), syntax).unwrap_or_default(),
            )
        })
    }

    fn notebook(path: &Path) -> Option<Preview> {
        let path_str = path.to_str()?;
        // nbconvert is bundled with jupyter, no need to check again
        let output = execute_and_capture_output_without_check(
            JUPYTER,
            &["nbconvert", "--to", "markdown", path_str, "--stdout"],
        )
        .ok()?;
        Self::syntaxed_from_str(output, "md")
    }

    fn syntaxed_from_str(output: String, ext: &str) -> Option<Preview> {
        let ss = SyntaxSet::load_defaults_nonewlines();
        ss.find_syntax_by_extension(ext).map(|syntax| {
            Preview::Syntaxed(HLContent::from_str(&output, ss.clone(), syntax).unwrap_or_default())
        })
    }

    fn text_or_binary(&self) -> Result<Preview> {
        let mut file = std::fs::File::open(self.file_info.path.clone())?;
        let mut buffer = vec![0; Self::CONTENT_INSPECTOR_MIN_SIZE];
        if self.is_binary(&mut file, &mut buffer) {
            Ok(Preview::Binary(BinaryContent::new(self.file_info)?))
        } else {
            Ok(Preview::Text(TextContent::from_file(&self.file_info.path)?))
        }
    }

    fn is_binary(&self, file: &mut std::fs::File, buffer: &mut [u8]) -> bool {
        self.file_info.true_size >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
            && file.read_exact(buffer).is_ok()
            && inspect(buffer) == ContentType::BINARY
    }

    /// Creates the help preview as if it was a text file.
    pub fn help(help: &str) -> Preview {
        Preview::Text(TextContent::help(help))
    }

    pub fn log(log: Vec<String>) -> Preview {
        Preview::Text(TextContent::log(log))
    }

    pub fn cli_info(output: &str, command: String) -> Preview {
        Preview::ColoredText(ColoredText::new(output, command))
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
        if let Ok(output) = execute_and_output_no_log(SS, ["-lpmepiT"]) {
            let s = String::from_utf8(output.stdout).unwrap_or_default();
            content = s
                .lines()
                .filter(|l| l.contains(&fileinfo.filename.to_string()))
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
    /// See `man lsblk` for a description of the arguments.
    fn new(fileinfo: &FileInfo) -> Self {
        let content: Vec<String>;
        if let Ok(output) = execute_and_output_no_log(
            LSBLK,
            [
                "-lfo",
                "FSTYPE,PATH,LABEL,UUID,FSVER,MOUNTPOINT,MODEL,SIZE,FSAVAIL,FSUSE%",
                &path_to_string(&fileinfo.path),
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
    fn new(fileinfo: &FileInfo) -> Self {
        let content: Vec<String>;
        if let Ok(output) =
            execute_and_output_no_log(LSOF, [path_to_string(&fileinfo.path).as_str()])
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
    /// Only files with less than 1MiB will be read
    const SIZE_LIMIT: usize = 1 << 20;

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
    /// Only files with less than 32kiB will be read
    const SIZE_LIMIT: usize = 1 << 15;

    /// Creates a new displayable content of a syntect supported file.
    /// It may fail if the file isn't properly formatted or the extension
    /// is wrong (ie. python content with .c extension).
    /// ATM only Monokaï (dark) theme is supported.
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

    fn get_or_init_monokai() -> &'static Theme {
        MONOKAI_THEME.get_or_init(|| {
            let mut monokai = BufReader::new(Cursor::new(include_bytes!(
                "../../../assets/themes/Monokai_Extended.tmTheme"
            )));
            ThemeSet::load_from_reader(&mut monokai).expect("Couldn't find monokai theme")
        })
    }

    fn parse_raw_content(
        raw_content: Vec<String>,
        syntax_set: SyntaxSet,
        syntax_ref: &SyntaxReference,
    ) -> Result<Vec<Vec<SyntaxedString>>> {
        let mut highlighted_content = vec![];
        let monokai = Self::get_or_init_monokai();
        let mut highlighter = HighlightLines::new(syntax_ref, monokai);

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
        let attr = Attr {
            fg: Color::Rgb(fg.r, fg.g, fg.b),
            bg: Color::default(),
            effect: Self::fontstyle_to_effect(&style.font_style),
        };
        Self { col, content, attr }
    }

    fn fontstyle_to_effect(font_style: &FontStyle) -> Effect {
        let mut effect = Effect::empty();

        // If the FontStyle has the bold bit set, add bold to the Effect
        if font_style.contains(FontStyle::BOLD) {
            effect |= Effect::BOLD;
        }

        // If the FontStyle has the underline bit set, add underline to the Effect
        if font_style.contains(FontStyle::UNDERLINE) {
            effect |= Effect::UNDERLINE;
        }

        effect
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
            }
            content.push(Line::new(buffer.into()));
            if content.len() >= Self::SIZE_LIMIT {
                break;
            }
        }

        Ok(Self {
            path: file_info.path.to_path_buf(),
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
        let mut hex_repr = String::new();
        for (i, byte) in self.line.iter().enumerate() {
            let _ = write!(hex_repr, "{byte:02x}");
            if i % 2 == 1 {
                hex_repr.push(' ');
            }
        }
        hex_repr
    }

    /// Converts a byte into '.' if it represent a non ASCII printable char
    /// or it's corresponding char.
    fn byte_to_char(byte: &u8) -> char {
        let ch = *byte as char;
        if !ch.is_ascii_graphic() {
            '.'
        } else {
            ch
        }
    }

    /// Format a line of 16 bytes as an ASCII string.
    /// Non ASCII printable bytes are replaced by dots.
    fn format_as_ascii(&self) -> String {
        self.line.iter().map(Self::byte_to_char).collect()
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
