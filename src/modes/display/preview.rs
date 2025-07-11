use std::cmp::min;
use std::convert::Into;
use std::fmt::{Display, Write as _};
use std::fs::symlink_metadata;
use std::io::{BufRead, BufReader, Read};
use std::iter::{Enumerate, Skip, Take};
use std::path::{Path, PathBuf};
use std::slice::Iter;

use anyhow::{Context, Result};
use content_inspector::{inspect, ContentType};
use ratatui::style::{Color, Modifier, Style};
use regex::Regex;
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style as SyntectStyle},
    parsing::{SyntaxReference, SyntaxSet},
};

use crate::common::{
    clear_tmp_files, filename_from_path, is_in_path, path_to_string, BSDTAR, FFMPEG, FONTIMAGE,
    ISOINFO, JUPYTER, LIBREOFFICE, LSBLK, MEDIAINFO, PANDOC, PDFINFO, PDFTOPPM, READELF,
    RSVG_CONVERT, SEVENZ, SS, TRANSMISSION_SHOW, UDEVADM,
};
use crate::config::get_syntect_theme;
use crate::io::execute_and_capture_output_without_check;
use crate::modes::{
    extract_extension, list_files_tar, list_files_zip, ContentWindow, DisplayedImage,
    DisplayedImageBuilder, FileKind, FilterKind, TLine, Tree, TreeBuilder, TreeLines, Users,
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
    Sevenz,
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
            "zip" | "gzip" | "bzip2" | "xz" | "lzip" | "lzma" | "tar" | "mtree" | "raw" | "gz" | "zst" | "deb" | "rpm"
            => Self::Archive,
            "7z" | "7za"
            => Self::Sevenz,
            "png" | "jpg" | "jpeg" | "tiff" | "heif" | "gif" | "cr2" | "nef" | "orf" | "sr2"
            => Self::Image,
            "ogg" | "ogm" | "riff" | "mp2" | "mp3" | "wm" | "qt" | "ac3" | "dts" | "aac" | "mac" | "flac"
            => Self::Audio,
            "mkv" | "webm" | "mpeg" | "mp4" | "avi" | "flv" | "mpg" | "wmv" | "m4v" | "mov"
            => Self::Video,
            "ttf" | "otf" | "woff"
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
            Self::Archive   => is_in_path(BSDTAR),
            Self::Epub      => is_in_path(PANDOC),
            Self::Iso       => is_in_path(ISOINFO),
            Self::Notebook  => is_in_path(JUPYTER),
            Self::Audio     => is_in_path(MEDIAINFO),
            Self::Office    => is_in_path(LIBREOFFICE),
            Self::Torrent   => is_in_path(TRANSMISSION_SHOW),
            Self::Sevenz    => is_in_path(SEVENZ),
            Self::Svg       => is_in_path(RSVG_CONVERT),
            Self::Video     => is_in_path(FFMPEG),
            Self::Font      => is_in_path(FONTIMAGE),
            Self::Pdf       => {
                               is_in_path(PDFINFO)
                            && is_in_path(PDFTOPPM)
            }

            _           => true,
        }
    }

    fn is_image_kind(&self) -> bool {
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
            Self::Sevenz    => write!(f, "7zip"),
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
    Image(DisplayedImage),
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
            Self::Image(preview) => preview.len(),
            Self::Tree(tree) => tree.displayable().lines().len(),
        }
    }

    pub fn kind_display(&self) -> &str {
        match self {
            Self::Empty => "empty",
            Self::Syntaxed(_) => "an highlighted text",
            Self::Text(text) => text.kind.for_first_line(),
            Self::Binary(_) => "a binary file",
            Self::Image(image) => image.kind.for_first_line(),
            Self::Tree(_) => "a tree",
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

    pub fn filepath(&self) -> String {
        match self {
            Self::Empty => "".to_owned(),
            Self::Syntaxed(preview) => preview.filepath().to_owned(),
            Self::Text(preview) => preview.title.to_owned(),
            Self::Binary(preview) => preview.path.to_string_lossy().to_string(),
            Self::Image(preview) => preview.identifier.to_owned(),
            Self::Tree(tree) => tree.root_path().to_string_lossy().to_string(),
        }
    }
}

/// Builder of previews. It just knows what file asked a preview.
/// Using a builder is useful since there's many kind of preview which all use a different method.
pub struct PreviewBuilder {
    path: PathBuf,
}

impl PreviewBuilder {
    const CONTENT_INSPECTOR_MIN_SIZE: usize = 1024;

    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
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
            FileKind::SymbolicLink(true) => self.valid_symlink(),
            _ => Ok(Preview::default()),
        }
    }

    /// Creates a new `Directory` from the self.file_info
    /// It explores recursivelly the directory and creates a tree.
    /// The recursive exploration is limited to depth 2.
    fn directory(&self) -> Result<Preview> {
        let users = Users::default();
        Ok(Preview::Tree(
            TreeBuilder::new(std::sync::Arc::from(self.path.as_path()), &users)
                .with_max_depth(4)
                .with_hidden(false)
                .with_filter_kind(&FilterKind::All)
                .build(),
        ))
    }

    fn valid_symlink(&self) -> Result<Preview> {
        Self::new(&std::fs::read_link(&self.path).unwrap_or_default()).build()
    }

    fn normal_file(&self) -> Result<Preview> {
        let extension = extract_extension(&self.path).to_lowercase();
        let kind = ExtensionKind::matcher(&extension);
        match kind {
            ExtensionKind::Archive if kind.has_programs() => {
                Ok(Preview::Text(Text::archive(&self.path, &extension)?))
            }
            ExtensionKind::Sevenz if kind.has_programs() => {
                Ok(Preview::Text(Text::sevenz(&self.path)?))
            }
            ExtensionKind::Iso if kind.has_programs() => Ok(Preview::Text(Text::iso(&self.path)?)),
            ExtensionKind::Epub if kind.has_programs() => Ok(Preview::Text(
                Text::epub(&self.path).context("Preview: Couldn't read epub")?,
            )),
            ExtensionKind::Torrent if kind.has_programs() => Ok(Preview::Text(
                Text::torrent(&self.path).context("Preview: Couldn't read torrent")?,
            )),
            ExtensionKind::Notebook if kind.has_programs() => {
                Ok(Self::notebook(&self.path).context("Preview: Couldn't parse notebook")?)
            }
            ExtensionKind::Audio if kind.has_programs() => {
                Ok(Preview::Text(Text::media_content(&self.path)?))
            }
            _ if kind.is_image_kind() && kind.has_programs() => Self::image(&self.path, kind),
            _ => match self.syntaxed(&extension) {
                Some(syntaxed_preview) => Ok(syntaxed_preview),
                None => self.text_or_binary(),
            },
        }
    }

    fn image(path: &Path, kind: ExtensionKind) -> Result<Preview> {
        let preview = DisplayedImageBuilder::new(path, kind.into()).build()?;
        if preview.is_empty() {
            Ok(Preview::Empty)
        } else {
            Ok(Preview::Image(preview))
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
            HLContent::from_str(
                "command".to_owned(),
                &output,
                ss.clone(),
                ss.find_syntax_by_extension(ext)?,
            )
            .unwrap_or_default(),
        ))
    }

    fn text_or_binary(&self) -> Result<Preview> {
        if let Some(elf) = self.read_elf() {
            Ok(Preview::Text(Text::from_readelf(&self.path, elf)?))
        } else if self.is_binary()? {
            Ok(Preview::Binary(BinaryContent::new(&self.path)?))
        } else {
            Ok(Preview::Text(Text::from_file(&self.path)?))
        }
    }

    fn read_elf(&self) -> Option<String> {
        let Ok(output) = execute_and_capture_output_without_check(
            READELF,
            &["-WCa", self.path.to_string_lossy().as_ref()],
        ) else {
            return None;
        };
        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    fn is_binary(&self) -> Result<bool> {
        let mut file = std::fs::File::open(&self.path)?;
        let mut buffer = [0; Self::CONTENT_INSPECTOR_MIN_SIZE];
        let Ok(metadata) = self.path.metadata() else {
            return Ok(false);
        };

        Ok(metadata.len() >= Self::CONTENT_INSPECTOR_MIN_SIZE as u64
            && file.read_exact(&mut buffer).is_ok()
            && inspect(&buffer) == ContentType::BINARY)
    }

    /// Creates the help preview as if it was a text file.
    pub fn help(help: &str) -> Preview {
        Preview::Text(Text::help(help))
    }

    pub fn log(log: Vec<String>) -> Preview {
        Preview::Text(Text::log(log))
    }

    pub fn cli_info(output: &str, command: String) -> Preview {
        crate::log_info!("cli_info. command {command} - output\n{output}");
        Preview::Text(Text::command_stdout(output, command))
    }
}

/// Reads a number of lines from a text file, _removing all ANSI control characters_.
/// Returns a vector of strings.
fn read_nb_lines(path: &Path, size_limit: usize) -> Result<Vec<String>> {
    let re = Regex::new(r"[[:cntrl:]]").unwrap();
    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    Ok(reader
        .lines()
        .take(size_limit)
        .map(|line| line.unwrap_or_default())
        .map(|s| re.replace_all(&s, "").to_string())
        .collect())
}

/// Different kind of text previewed.
/// Wether it's a text file or the output of a command.
#[derive(Clone, Default, Debug)]
pub enum TextKind {
    #[default]
    TEXTFILE,

    Archive,
    Blockdevice,
    CommandStdout,
    Elf,
    Epub,
    FifoChardevice,
    Help,
    Iso,
    Log,
    Mediacontent,
    Sevenz,
    Socket,
    Torrent,
}

impl TextKind {
    /// Used to display the kind of content in this file.
    pub fn for_first_line(&self) -> &'static str {
        match self {
            Self::TEXTFILE => "a textfile",
            Self::Archive => "an archive",
            Self::Blockdevice => "a Blockdevice file",
            Self::CommandStdout => "a command stdout",
            Self::Elf => "an elf file",
            Self::Epub => "an epub",
            Self::FifoChardevice => "a Fifo or Chardevice file",
            Self::Help => "Help",
            Self::Iso => "Iso",
            Self::Log => "Log",
            Self::Mediacontent => "a media content",
            Self::Sevenz => "a 7z archive",
            Self::Socket => "a Socket file",
            Self::Torrent => "a torrent",
        }
    }
}

impl Display for TextKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{kind_str}", kind_str = self.for_first_line())
    }
}

/// Holds a preview of a text content.
/// It's a vector of strings (per line)
#[derive(Clone, Default, Debug)]
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

    fn from_readelf(path: &Path, elf: String) -> Result<Self> {
        Ok(Self {
            title: filename_from_path(path).context("")?.to_owned(),
            kind: TextKind::Elf,
            length: elf.len(),
            content: elf.lines().map(|line| line.to_owned()).collect(),
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

    fn sevenz(path: &Path) -> Result<Self> {
        Self::from_command_output(TextKind::Sevenz, SEVENZ, &["l", &path_to_string(&path)])
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
    pub fn command_stdout(output: &str, title: String) -> Self {
        let content: Vec<String> = output.lines().map(|line| line.to_owned()).collect();
        let length = content.len();
        Self {
            title,
            kind: TextKind::CommandStdout,
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
    path: String,
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
        Self::build(
            path.to_string_lossy().to_string(),
            raw_content,
            syntax_set,
            syntax_ref,
        )
    }

    fn from_str(
        name: String,
        text: &str,
        syntax_set: SyntaxSet,
        syntax_ref: &SyntaxReference,
    ) -> Result<Self> {
        let raw_content = text
            .lines()
            .take(Self::SIZE_LIMIT)
            .map(|s| s.to_owned())
            .collect();
        Self::build(name, raw_content, syntax_set, syntax_ref)
    }

    fn build(
        path: String,
        raw_content: Vec<String>,
        syntax_set: SyntaxSet,
        syntax_ref: &SyntaxReference,
    ) -> Result<Self> {
        let highlighted_content = Self::parse_raw_content(raw_content, syntax_set, syntax_ref)?;
        Ok(Self {
            path,
            length: highlighted_content.len(),
            content: highlighted_content,
        })
    }

    fn len(&self) -> usize {
        self.length
    }

    fn filepath(&self) -> &str {
        &self.path
    }

    fn parse_raw_content(
        raw_content: Vec<String>,
        syntax_set: SyntaxSet,
        syntax_ref: &SyntaxReference,
    ) -> Result<Vec<Vec<SyntaxedString>>> {
        let mut highlighted_content = vec![];
        let syntect_theme = get_syntect_theme().context("Syntect set should be set")?;
        let mut highlighter = HighlightLines::new(syntax_ref, syntect_theme);

        for line in raw_content.iter() {
            let mut v_line = vec![];
            if let Ok(v) = highlighter.highlight_line(line, &syntax_set) {
                for (style, token) in v.iter() {
                    v_line.push(SyntaxedString::from_syntect(token, *style));
                }
            }
            highlighted_content.push(v_line)
        }

        Ok(highlighted_content)
    }
}

/// Holds a string to be displayed with given .
/// We have to read the  from Syntect and parse it into ratatui attr
/// This struct does the parsing.
#[derive(Clone)]
pub struct SyntaxedString {
    pub content: String,
    pub style: Style,
}

impl SyntaxedString {
    /// Parse a content and style into a `SyntaxedString`
    /// Only the foreground color is read, we don't the background nor
    /// the style (bold, italic, underline) defined in Syntect.
    fn from_syntect(content: &str, style: SyntectStyle) -> Self {
        let content = content.to_owned();
        let fg = style.foreground;
        let style = Style {
            fg: Some(Color::Rgb(fg.r, fg.g, fg.b)),
            bg: None,
            add_modifier: Self::font_style_to_effect(&style.font_style),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        };
        Self { content, style }
    }

    fn font_style_to_effect(font_style: &FontStyle) -> Modifier {
        let mut modifier = Modifier::empty();

        // If the FontStyle has the bold bit set, add bold to the Effect
        if font_style.contains(FontStyle::BOLD) {
            modifier |= Modifier::BOLD;
        }

        // If the FontStyle has the underline bit set, add underline to the Modifier
        if font_style.contains(FontStyle::UNDERLINE) {
            modifier |= Modifier::UNDERLINED;
        }

        modifier
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
        let length = metadata.len() / Self::LINE_WIDTH as u64;
        let content = Self::read_content(path)?;

        Ok(Self {
            path: path.to_path_buf(),
            length,
            content,
        })
    }

    fn read_content(path: &Path) -> Result<Vec<Line>> {
        let mut reader = BufReader::new(std::fs::File::open(path)?);
        let mut buffer = [0; Self::LINE_WIDTH];
        let mut content = vec![];
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
        Ok(content)
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

    pub fn number_width_hex(&self) -> usize {
        format!("{:x}", self.len() * 16).len()
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
    pub fn format_hex(&self) -> String {
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
        if ch.is_ascii_graphic() {
            ch
        } else {
            '.'
        }
    }

    /// Format a line of 16 bytes as an ASCII string.
    /// Non ASCII printable bytes are replaced by dots.
    pub fn format_as_ascii(&self) -> String {
        self.line.iter().map(Self::byte_to_char).collect()
    }

    pub fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
        format!("{line_nr:0width$x}  ")
    }
}

/// Common trait for many preview methods which are just a bunch of lines with
/// no specific formatting.
/// Some previewing (thumbnail and syntaxed text) needs more details.
pub trait TakeSkip<T> {
    fn take_skip(&self, top: usize, bottom: usize, length: usize) -> Take<Skip<Iter<'_, T>>>;
}

macro_rules! impl_take_skip {
    ($t:ident, $u:ident) => {
        impl TakeSkip<$u> for $t {
            fn take_skip(
                &self,
                top: usize,
                bottom: usize,
                length: usize,
            ) -> Take<Skip<Iter<'_, $u>>> {
                self.content.iter().skip(top).take(min(length, bottom + 1))
            }
        }
    };
}
/// Common trait for many preview methods which are just a bunch of lines with
/// no specific formatting.
/// Some previewing (thumbnail and syntaxed text) needs more details.
pub trait TakeSkipEnum<T> {
    fn take_skip_enum(
        &self,
        top: usize,
        bottom: usize,
        length: usize,
    ) -> Take<Skip<Enumerate<Iter<'_, T>>>>;
}

macro_rules! impl_take_skip_enum {
    ($t:ident, $u:ident) => {
        impl TakeSkipEnum<$u> for $t {
            fn take_skip_enum(
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

impl_take_skip_enum!(HLContent, VecSyntaxedString);
impl_take_skip_enum!(Text, String);
impl_take_skip_enum!(BinaryContent, Line);
impl_take_skip_enum!(TreeLines, TLine);

impl_take_skip!(HLContent, VecSyntaxedString);
impl_take_skip!(Text, String);
impl_take_skip!(BinaryContent, Line);
impl_take_skip!(TreeLines, TLine);
