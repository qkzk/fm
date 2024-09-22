use std::cmp::min;
use std::convert::Into;
use std::fmt::Write as _;
use std::fs::symlink_metadata;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::iter::{Enumerate, Skip, Take};
use std::path::{Path, PathBuf};
use std::slice::Iter;

use anyhow::{Context, Result};
use content_inspector::{inspect, ContentType};
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style, Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};
use tuikit::attr::{Attr, Color, Effect};

use crate::common::{
    clear_tmp_files, filename_from_path, is_in_path, path_to_string, FFMPEG, FONTIMAGE, ISOINFO,
    JUPYTER, LIBREOFFICE, LSBLK, MEDIAINFO, PANDOC, PDFINFO, PDFTOPPM, RSVG_CONVERT, SS,
    TRANSMISSION_SHOW, UDEVADM, UEBERZUG,
};
use crate::config::MONOKAI_THEME;
use crate::io::execute_and_capture_output_without_check;
use crate::modes::{
    extract_extension, list_files_tar, list_files_zip, read_symlink_dest, ContentWindow, FileKind,
    FilterKind, SortKind, Tree, TreeLineBuilder, TreeLines, Ueber, UeberBuilder, Users,
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
    #[rustfmt::skip]
    pub fn matcher(ext: &str) -> Self {
        match ext {
            "zip" | "gzip" | "bzip2" | "xz" | "lzip" | "lzma" | "tar" | "mtree" | "raw" | "7z" | "gz" | "zst" | "deb" | "rpm"
            => Self::Archive,
            "png" | "jpg" | "jpeg" | "tiff" | "heif" | "gif" | "cr2" | "nef" | "orf" | "sr2"
            => Self::Image,
            "ogg" | "ogm" | "riff" | "mp2" | "mp3" | "wm" | "qt" | "ac3" | "dts" | "aac" | "mac" | "flac"
            => Self::Audio,
            "mkv" | "webm" | "mpeg" | "mp4" | "avi" | "flv" | "mpg" | "wmv" | "m4v" | "mov"
            => Self::Video,
            "ttf" | "otf"
            => Self::Font,
            "svg" | "svgz"
            => Self::Svg,
            "pdf"
            => Self::Pdf,
            "iso"
            => Self::Iso,
            "ipynb"
            => Self::Notebook,
            "doc" | "docx" | "odt" | "sxw" | "xlsx" | "xls" 
            => Self::Office,
            "epub"
            => Self::Epub,
            "torrent"
            => Self::Torrent,
            _
            => Self::Default,
        }
    }

    #[rustfmt::skip]
    fn has_programs(&self) -> bool {
        match self {
            Self::Epub      => is_in_path(PANDOC),
            Self::Iso       => is_in_path(ISOINFO),
            Self::Notebook  => is_in_path(JUPYTER),
            Self::Audio     => is_in_path(MEDIAINFO),
            Self::Office    => is_in_path(LIBREOFFICE),
            Self::Torrent   => is_in_path(TRANSMISSION_SHOW),
            Self::Image     => is_in_path(UEBERZUG),
            Self::Svg       => is_in_path(UEBERZUG) && is_in_path(RSVG_CONVERT),
            Self::Video     => is_in_path(UEBERZUG) && is_in_path(FFMPEG),
            Self::Font      => is_in_path(UEBERZUG) && is_in_path(FONTIMAGE),
            Self::Pdf       => {
                               is_in_path(UEBERZUG)
                            && is_in_path(PDFINFO)
                            && is_in_path(PDFTOPPM)
            }

            _           => true,
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

/// Different kind of preview used to display some informaitons
/// About the file.
/// We check if it's an archive first, then a pdf file, an image, a media file
#[derive(Default)]
pub enum Preview {
    Syntaxed(HLContent),
    Text(Text),
    Binary(BinaryContent),
    Ueberzug(Ueber),
    Tree(Tree),
    #[default]
    Empty,
}

impl Preview {
    /// The size (most of the time the number of lines) of the preview.
    /// Some preview (thumbnail, empty) can't be scrolled and their size is always 0.
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Syntaxed(preview) => preview.len(),
            Self::Text(preview) => preview.len(),
            Self::Binary(preview) => preview.len(),
            Self::Ueberzug(preview) => preview.len(),
            Self::Tree(tree) => tree.displayable().lines().len(),
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
    path: PathBuf,
    users: &'a Users,
}

impl<'a> PreviewBuilder<'a> {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    pub fn new(path: &Path, users: &'a Users) -> Self {
        Self {
            path: path.to_owned(),
            users,
        }
    }

    /// Empty preview, holding nothing.
    pub fn empty() -> Preview {
        clear_tmp_files();
        Preview::Empty
    }

    /// Creates a new preview instance based on the filekind and the extension of
    /// the file.
    /// Sometimes it reads the content of the file, sometimes it delegates
    /// it to the display method.
    /// Directories aren't handled there since we need more arguments to create
    /// their previews.
    pub fn build(self) -> Result<Preview> {
        clear_tmp_files();
        let file_kind = FileKind::new(&symlink_metadata(&self.path)?, &self.path);
        match file_kind {
            FileKind::Directory => self.directory(),
            FileKind::NormalFile => self.normal_file(),
            FileKind::Socket if is_in_path(SS) => self.socket(),
            FileKind::BlockDevice if is_in_path(LSBLK) => self.block_device(),
            FileKind::Fifo | FileKind::CharDevice if is_in_path(UDEVADM) => self.fifo_chardevice(),
            FileKind::SymbolicLink(valid) if valid => self.valid_symlink(),
            _ => Ok(Preview::default()),
        }
    }

    /// Creates a new `Directory` from the self.file_info
    /// It explores recursivelly the directory and creates a tree.
    /// The recursive exploration is limited to depth 2.
    fn directory(&self) -> Result<Preview> {
        Ok(Preview::Tree(Tree::new(
            std::sync::Arc::from(self.path.as_path()),
            4,
            SortKind::tree_default(),
            self.users,
            false,
            &FilterKind::All,
        )))
    }

    fn valid_symlink(&self) -> Result<Preview> {
        let dest = read_symlink_dest(&self.path).context("broken symlink")?;
        let dest_path = Path::new(&dest);
        Self::new(dest_path, self.users).build()
    }

    fn normal_file(&self) -> Result<Preview> {
        let extension = extract_extension(&self.path).to_lowercase();
        let path = &self.path;
        let kind = ExtensionKind::matcher(&extension);
        match kind {
            ExtensionKind::Archive => Ok(Preview::Text(Text::archive(&self.path, &extension)?)),
            ExtensionKind::Iso if kind.has_programs() => Ok(Preview::Text(Text::iso(path)?)),
            ExtensionKind::Epub if kind.has_programs() => Ok(Preview::Text(
                Text::epub(path).context("Preview: Couldn't read epub")?,
            )),
            ExtensionKind::Torrent if kind.has_programs() => Ok(Preview::Text(
                Text::torrent(path).context("Preview: Couldn't read torrent")?,
            )),
            ExtensionKind::Notebook if kind.has_programs() => {
                Ok(Self::notebook(path).context("Preview: Couldn't parse notebook")?)
            }
            ExtensionKind::Audio if kind.has_programs() => {
                Ok(Preview::Text(Text::media_content(path)?))
            }
            _ if kind.is_ueber_kind() && kind.has_programs() => Self::ueber(path, kind),
            _ => match self.syntaxed(&extension) {
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

    fn socket(&self) -> Result<Preview> {
        Ok(Preview::Text(Text::socket(&self.path)?))
    }

    fn block_device(&self) -> Result<Preview> {
        Ok(Preview::Text(Text::block_device(&self.path)?))
    }

    fn fifo_chardevice(&self) -> Result<Preview> {
        Ok(Preview::Text(Text::fifo_chardevice(&self.path)?))
    }

    fn syntaxed(&self, ext: &str) -> Option<Preview> {
        if symlink_metadata(&self.path).ok()?.len() > HLContent::SIZE_LIMIT as u64 {
            return None;
        };
        let ss = SyntaxSet::load_defaults_nonewlines();
        Some(Preview::Syntaxed(
            HLContent::new(&self.path, ss.clone(), ss.find_syntax_by_extension(ext)?)
                .unwrap_or_default(),
        ))
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
        Some(Preview::Syntaxed(
            HLContent::from_str(&output, ss.clone(), ss.find_syntax_by_extension(ext)?)
                .unwrap_or_default(),
        ))
    }

    fn text_or_binary(&self) -> Result<Preview> {
        let mut file = std::fs::File::open(&self.path)?;
        let mut buffer = vec![0; Self::CONTENT_INSPECTOR_MIN_SIZE];
        if self.is_binary(&mut file, &mut buffer) {
            Ok(Preview::Binary(BinaryContent::new(&self.path)?))
        } else {
            Ok(Preview::Text(Text::from_file(&self.path)?))
        }
    }

    fn is_binary(&self, file: &mut std::fs::File, buffer: &mut [u8]) -> bool {
        let Ok(metadata) = self.path.metadata() else {
            return false;
        };

        metadata.len() >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
            && file.read_exact(buffer).is_ok()
            && inspect(buffer) == ContentType::BINARY
    }

    /// Creates the help preview as if it was a text file.
    pub fn help(help: &str) -> Preview {
        Preview::Text(Text::help(help))
    }

    pub fn log(log: Vec<String>) -> Preview {
        Preview::Text(Text::log(log))
    }

    pub fn cli_info(output: &str, command: String) -> Preview {
        Preview::Text(Text::cli_info(output, command))
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

#[derive(Clone, Default)]
pub enum TextKind {
    #[default]
    TEXTFILE,

    Archive,
    Blockdevice,
    CliInfo,
    Epub,
    FifoChardevice,
    Help,
    Iso,
    Log,
    Mediacontent,
    Socket,
    Torrent,
}

/// Holds a preview of a text content.
/// It's a vector of strings (per line)
#[derive(Clone, Default)]
pub struct Text {
    pub kind: TextKind,
    pub title: String,
    content: Vec<String>,
    length: usize,
}

impl Text {
    /// Only files with less than 1MiB will be read
    const SIZE_LIMIT: usize = 1 << 20;

    fn help(help: &str) -> Self {
        let content: Vec<String> = help.lines().map(|line| line.to_owned()).collect();
        Self {
            title: "Help".to_string(),
            kind: TextKind::Help,
            length: content.len(),
            content,
        }
    }

    fn log(content: Vec<String>) -> Self {
        Self {
            title: "Logs".to_string(),
            kind: TextKind::Log,
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
            title: "Epub".to_string(),
            kind: TextKind::Epub,
            length: content.len(),
            content,
        })
    }

    fn from_file(path: &Path) -> Result<Self> {
        let content = read_nb_lines(path, Self::SIZE_LIMIT)?;
        Ok(Self {
            title: filename_from_path(path).context("")?.to_owned(),
            kind: TextKind::TEXTFILE,
            length: content.len(),
            content,
        })
    }

    fn from_command_output(kind: TextKind, command: &str, args: &[&str]) -> Result<Self> {
        let content: Vec<String> = execute_and_capture_output_without_check(command, args)?
            .lines()
            .map(|s| s.to_owned())
            .collect();
        Ok(Self {
            title: command.to_owned(),
            kind,
            length: content.len(),
            content,
        })
    }

    fn media_content(path: &Path) -> Result<Self> {
        Self::from_command_output(
            TextKind::Mediacontent,
            MEDIAINFO,
            &[path_to_string(&path).as_str()],
        )
    }

    /// Holds a list of file of an archive as returned by
    /// `ZipArchive::file_names` or from  a `tar tvf` command.
    /// A generic error message prevent it from returning an error.
    fn archive(path: &Path, ext: &str) -> Result<Self> {
        let content = match ext {
            "zip" => list_files_zip(path).unwrap_or(vec!["Invalid Zip content".to_owned()]),
            "zst" | "gz" | "bz" | "xz" | "gzip" | "bzip2" | "deb" | "rpm" => {
                list_files_tar(path).unwrap_or(vec!["Invalid Tar content".to_owned()])
            }
            _ => vec![format!("Unsupported format: {ext}")],
        };

        Ok(Self {
            title: filename_from_path(path).context("")?.to_owned(),
            kind: TextKind::Archive,
            length: content.len(),
            content,
        })
    }

    fn iso(path: &Path) -> Result<Self> {
        Self::from_command_output(
            TextKind::Iso,
            ISOINFO,
            &["-l", "-i", &path_to_string(&path)],
        )
    }

    fn torrent(path: &Path) -> Result<Self> {
        Self::from_command_output(
            TextKind::Torrent,
            TRANSMISSION_SHOW,
            &[&path_to_string(&path)],
        )
    }

    /// New socket preview
    /// See `man ss` for a description of the arguments.
    fn socket(path: &Path) -> Result<Self> {
        let mut preview = Self::from_command_output(TextKind::Socket, SS, &["-lpmepiT"])?;
        preview.content = preview
            .content
            .iter()
            .filter(|l| l.contains(path.file_name().unwrap().to_string_lossy().as_ref()))
            .map(|s| s.to_owned())
            .collect();
        Ok(preview)
    }

    /// New blockdevice preview
    /// See `man lsblk` for a description of the arguments.
    fn block_device(path: &Path) -> Result<Self> {
        Self::from_command_output(
            TextKind::Blockdevice,
            LSBLK,
            &[
                "-lfo",
                "FSTYPE,PATH,LABEL,UUID,FSVER,MOUNTPOINT,MODEL,SIZE,FSAVAIL,FSUSE%",
                &path_to_string(&path),
            ],
        )
    }

    /// New FIFO preview
    /// See `man udevadm` for a description of the arguments.
    fn fifo_chardevice(path: &Path) -> Result<Self> {
        Self::from_command_output(
            TextKind::FifoChardevice,
            UDEVADM,
            &[
                "info",
                "-a",
                "-n",
                path_to_string(&path).as_str(),
                "--no-pager",
            ],
        )
    }
    /// Make a new previewed colored text.
    pub fn cli_info(output: &str, title: String) -> Self {
        let content: Vec<String> = output.lines().map(|line| line.to_owned()).collect();
        let length = content.len();
        Self {
            title,
            kind: TextKind::CliInfo,
            content,
            length,
        }
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
        Self::build(raw_content, syntax_set, syntax_ref)
    }

    fn from_str(text: &str, syntax_set: SyntaxSet, syntax_ref: &SyntaxReference) -> Result<Self> {
        let raw_content = text
            .lines()
            .take(Self::SIZE_LIMIT)
            .map(|s| s.to_owned())
            .collect();
        Self::build(raw_content, syntax_set, syntax_ref)
    }

    fn build(
        raw_content: Vec<String>,
        syntax_set: SyntaxSet,
        syntax_ref: &SyntaxReference,
    ) -> Result<Self> {
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
#[derive(Clone, Default)]
pub struct BinaryContent {
    pub path: PathBuf,
    length: u64,
    content: Vec<Line>,
}

impl BinaryContent {
    const LINE_WIDTH: usize = 16;
    const SIZE_LIMIT: usize = 1048576;

    fn new(path: &Path) -> Result<Self> {
        let Ok(metadata) = path.metadata() else {
            return Ok(Self::default());
        };
        let size = metadata.len();
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

        Ok(Self {
            path: path.to_path_buf(),
            length: size / Self::LINE_WIDTH as u64,
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
impl_window!(Text, String);
impl_window!(BinaryContent, Line);
impl_window!(ColoredText, String);
impl_window!(TreeLines, TreeLineBuilder);
