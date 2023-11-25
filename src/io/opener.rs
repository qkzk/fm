use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde_yaml;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::common::is_program_in_path;
use crate::common::{
    DEFAULT_AUDIO_OPENER, DEFAULT_IMAGE_OPENER, DEFAULT_OFFICE_OPENER, DEFAULT_OPENER,
    DEFAULT_READABLE_OPENER, DEFAULT_TEXT_OPENER, DEFAULT_VECTORIAL_OPENER, DEFAULT_VIDEO_OPENER,
};
use crate::io::execute_in_child;
use crate::log_info;
use crate::modes::extract_extension;
use crate::modes::{decompress_gz, decompress_xz, decompress_zip};

/// Different kind of extensions for default openers.
#[derive(Clone, Hash, Eq, PartialEq, Debug, Display, Default, EnumString, EnumIter)]
enum ExtensionKind {
    #[default]
    Audio,
    Bitmap,
    Office,
    Readable,
    Text,
    Default,
    Vectorial,
    Video,
    Internal(InternalVariant),
}

// TODO: move those associations to a config file
impl ExtensionKind {
    fn matcher(ext: &str) -> Self {
        match ext {
            "avif" | "bmp" | "gif" | "png" | "jpg" | "jpeg" | "pgm" | "ppm" | "webp" | "tiff" => {
                Self::Bitmap
            }

            "svg" => Self::Vectorial,

            "flac" | "m4a" | "wav" | "mp3" | "ogg" | "opus" => Self::Audio,

            "avi" | "mkv" | "av1" | "m4v" | "ts" | "webm" | "mov" | "wmv" => Self::Video,

            "build" | "c" | "cmake" | "conf" | "cpp" | "css" | "csv" | "cu" | "ebuild" | "eex"
            | "env" | "ex" | "exs" | "go" | "h" | "hpp" | "hs" | "html" | "ini" | "java" | "js"
            | "json" | "kt" | "lua" | "log" | "md" | "micro" | "ninja" | "py" | "rkt" | "rs"
            | "scss" | "sh" | "srt" | "svelte" | "tex" | "toml" | "tsx" | "txt" | "vim" | "xml"
            | "yaml" | "yml" => Self::Text,

            "odt" | "odf" | "ods" | "odp" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => {
                Self::Office
            }

            "pdf" | "epub" => Self::Readable,

            "zip" => Self::Internal(InternalVariant::DecompressZip),

            "xz" | "7z" => Self::Internal(InternalVariant::DecompressXz),

            "lzip" | "lzma" | "rar" | "tgz" | "gz" | "bzip2" => {
                Self::Internal(InternalVariant::DecompressGz)
            }
            // iso files can't be mounted without more information than we hold in this enum :
            // we need to be able to change the status of the application to ask for a sudo password.
            // we can't use the "basic" opener to mount them.
            // ATM this is the only extension we can't open, it may change in the future.
            "iso" => {
                log_info!("extension kind iso");
                Self::Internal(InternalVariant::NotSupported)
            }
            _ => Self::Default,
        }
    }
}

macro_rules! open_file_with {
    ($self:ident, $key:expr, $variant:ident, $yaml:ident) => {
        if let Some(o) = OpenerInfo::from_yaml(&$yaml[$key]) {
            $self
                .association
                .entry(ExtensionKind::$variant)
                .and_modify(|e| *e = o);
        }
    };
}

/// Holds an association map between `ExtensionKind` and `OpenerInfo`.
/// It's used to know how to open a kind of file.
#[derive(Clone)]
pub struct OpenerAssociation {
    association: HashMap<ExtensionKind, OpenerInfo>,
}

impl OpenerAssociation {
    fn new() -> Self {
        Self {
            association: HashMap::from([
                (
                    ExtensionKind::Audio,
                    OpenerInfo::from_external(DEFAULT_AUDIO_OPENER),
                ),
                (
                    ExtensionKind::Bitmap,
                    OpenerInfo::from_external(DEFAULT_IMAGE_OPENER),
                ),
                (
                    ExtensionKind::Office,
                    OpenerInfo::from_external(DEFAULT_OFFICE_OPENER),
                ),
                (
                    ExtensionKind::Readable,
                    OpenerInfo::from_external(DEFAULT_READABLE_OPENER),
                ),
                (
                    ExtensionKind::Text,
                    OpenerInfo::from_external(DEFAULT_TEXT_OPENER),
                ),
                (
                    ExtensionKind::Vectorial,
                    OpenerInfo::from_external(DEFAULT_VECTORIAL_OPENER),
                ),
                (
                    ExtensionKind::Video,
                    OpenerInfo::from_external(DEFAULT_VIDEO_OPENER),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::DecompressZip),
                    OpenerInfo::from_internal(ExtensionKind::Internal(
                        InternalVariant::DecompressZip,
                    ))
                    .unwrap_or_default(),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::DecompressGz),
                    OpenerInfo::from_internal(ExtensionKind::Internal(
                        InternalVariant::DecompressGz,
                    ))
                    .unwrap_or_default(),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::DecompressXz),
                    OpenerInfo::from_internal(ExtensionKind::Internal(
                        InternalVariant::DecompressXz,
                    ))
                    .unwrap_or_default(),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::NotSupported),
                    OpenerInfo::from_internal(ExtensionKind::Internal(
                        InternalVariant::NotSupported,
                    ))
                    .unwrap_or_default(),
                ),
                (
                    ExtensionKind::Default,
                    OpenerInfo::from_external(DEFAULT_OPENER),
                ),
            ]),
        }
    }

    /// Converts itself into an hashmap of strings.
    /// Used to include openers in the help
    pub fn as_map_of_strings(&self) -> std::collections::HashMap<String, String> {
        let mut associations: std::collections::HashMap<String, String> = self
            .association
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        for s in ExtensionKind::iter() {
            let s = s.to_string();
            associations.entry(s).or_insert_with(|| "".to_owned());
        }
        associations
    }

    fn opener_info(&self, ext: &str) -> Option<&OpenerInfo> {
        self.association
            .get(&ExtensionKind::matcher(&ext.to_lowercase()))
    }

    fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        open_file_with!(self, "audio", Audio, yaml);
        open_file_with!(self, "bitmap_image", Bitmap, yaml);
        open_file_with!(self, "libreoffice", Office, yaml);
        open_file_with!(self, "readable", Readable, yaml);
        open_file_with!(self, "text", Text, yaml);
        open_file_with!(self, "default", Default, yaml);
        open_file_with!(self, "vectorial_image", Vectorial, yaml);
        open_file_with!(self, "video", Video, yaml);

        self.validate_openers();
        log_info!("update from file");
    }

    fn validate_openers(&mut self) {
        self.association
            .retain(|_, opener_info| opener_info.is_valid());
    }
}

/// Some kind of files are "opened" using internal methods.
/// ATM only one kind of files is supported, compressed ones, which use
/// libarchive internally.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Default)]
pub enum InternalVariant {
    #[default]
    DecompressZip,
    DecompressXz,
    DecompressGz,
    NotSupported,
}

impl InternalVariant {
    fn open(&self, filepath: &Path) -> Result<()> {
        match self {
            Self::DecompressZip => decompress_zip(filepath),
            Self::DecompressXz => decompress_xz(filepath),
            Self::DecompressGz => decompress_gz(filepath),
            Self::NotSupported => Err(anyhow!("Can't be opened directly")),
        }
    }
}

/// A way to open one kind of files.
/// It's either an internal method or an external program.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum OpenerInfo {
    Internal(InternalVariant),
    External(String, bool),
}

impl Default for OpenerInfo {
    fn default() -> Self {
        Self::from_external(DEFAULT_OPENER)
    }
}

impl OpenerInfo {
    fn from_external(opener_pair: (&str, bool)) -> Self {
        let opener = opener_pair.0.to_owned();
        let use_term = opener_pair.1;
        Self::External(opener, use_term)
    }

    fn from_internal(extension_kind: ExtensionKind) -> Result<Self> {
        if let ExtensionKind::Internal(internal) = extension_kind {
            return Ok(Self::Internal(internal));
        } else {
            return Err(anyhow!("Should be an internval variant"));
        }
    }

    fn from_yaml(yaml: &serde_yaml::value::Value) -> Option<Self> {
        Some(Self::from_external((
            yaml.get("opener")?.as_str()?,
            yaml.get("use_term")?.as_bool()?,
        )))
    }

    fn is_external(&self) -> bool {
        matches!(self, Self::External(_, _))
    }

    fn is_valid(&self) -> bool {
        !self.is_external() || is_program_in_path(self.external().unwrap_or_default().0)
    }

    fn external(&self) -> Result<(&str, bool)> {
        let Self::External(program, use_term) = self else {
            return Err(anyhow!("not an external opener"));
        };
        return Ok((program, *use_term));
    }

    fn open_internal(&self, filepath: &Path) -> Result<()> {
        let Self::Internal(internal_variant) = self else {
            return Err(anyhow!("should be an internal variant"));
        };
        internal_variant.open(filepath)
    }
}

impl fmt::Display for OpenerInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
        let s = if let Self::External(external, _) = &self {
            external
        } else {
            ""
        };
        write!(f, "{s}")
    }
}

/// Holds the associations between different kind of files and opener method
/// as well as the name of the terminal configured by the user.
#[derive(Clone)]
pub struct Opener {
    /// The name of the configured terminal application
    pub terminal: String,
    /// The association of openers for every kind of files
    pub opener_association: OpenerAssociation,
}

impl Opener {
    /// Creates a new opener instance.
    /// Default values are used. It may be uptaded with configured ones later.
    pub fn new(terminal: &str) -> Self {
        Self {
            terminal: terminal.to_owned(),
            opener_association: OpenerAssociation::new(),
        }
    }

    /// Open a file, using the configured method.
    /// It may fail if the program changed after reading the config file.
    /// It may also fail if the program can't handle this kind of files.
    /// This is quite a tricky method, there's many possible failures.
    pub fn open(&self, filepath: &Path) -> Result<()> {
        if filepath.is_dir() {
            return Err(anyhow!("open can't execute a directory"));
        }
        let extension = extract_extension(filepath);
        let Some(open_info) = self.get_opener(extension) else {
            return Err(anyhow!(
                "no opener defined for {filepath}",
                filepath = filepath.display()
            ));
        };
        if open_info.is_external() {
            self.open_external(filepath, open_info)?;
        } else {
            open_info.open_internal(filepath)?;
        }
        Ok(())
    }

    /// Open multiple files.
    /// Files sharing an opener are opened in a single command ie.: `nvim a.txt b.rs c.py`.
    /// Only files opened with an external opener are supported.
    pub fn open_multiple(&self, file_paths: &[PathBuf]) -> Result<()> {
        for (open_info, file_paths) in &self.regroup_openers(file_paths) {
            self.open_grouped_files(open_info, file_paths)?;
        }
        Ok(())
    }

    /// Returns the open info about this file.
    /// It's used to check if the file can be opened without specific actions or not.
    /// This opener can't mutate the status and can't ask for a sudo password.
    /// Some files requires root to be opened (ie. ISO files which are mounted).
    pub fn open_info(&self, filepath: &Path) -> Option<&OpenerInfo> {
        self.get_opener(extract_extension(filepath))
    }

    fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        self.opener_association.update_from_file(yaml)
    }

    fn get_opener(&self, extension: &str) -> Option<&OpenerInfo> {
        self.opener_association.opener_info(extension)
    }

    /// Create an hashmap of openers -> [files].
    /// Each file in the collection share the same opener.
    fn regroup_openers(&self, file_paths: &[PathBuf]) -> HashMap<OpenerInfo, Vec<PathBuf>> {
        let mut openers: HashMap<OpenerInfo, Vec<PathBuf>> = HashMap::new();
        for file_path in file_paths {
            let Some(open_info) = self.get_opener(extract_extension(file_path)) else {
                continue;
            };
            if open_info.is_external() {
                openers
                    .entry(open_info.to_owned())
                    .and_modify(|files| files.push((*file_path).to_owned()))
                    .or_insert(vec![(*file_path).to_owned()]);
            }
        }
        openers
    }

    /// Convert a slice of `PathBuf` into their string representation.
    /// Files which are directory are skipped.
    fn collect_paths_as_str(file_paths: &[PathBuf]) -> Vec<&str> {
        file_paths
            .iter()
            .filter(|fp| !fp.is_dir())
            .filter_map(|fp| fp.to_str())
            .collect()
    }

    fn open_grouped_files(&self, open_info: &OpenerInfo, file_paths: &[PathBuf]) -> Result<()> {
        let file_paths_str = Self::collect_paths_as_str(file_paths);
        let (external_program, use_term) = open_info.external()?;
        let mut args: Vec<&str> = vec![external_program];
        args.extend(&file_paths_str);
        self.open_with_args(args, use_term)?;
        Ok(())
    }

    fn open_external(&self, filepath: &Path, open_info: &OpenerInfo) -> Result<()> {
        let (external_program, use_term) = open_info.external()?;
        self.open_with(external_program, use_term, filepath)?;
        Ok(())
    }

    /// Open a file with a given program.
    /// If the program requires a terminal, the terminal itself is opened
    /// and the program and its parameters are sent to it.
    fn open_with(
        &self,
        program: &str,
        use_term: bool,
        filepath: &std::path::Path,
    ) -> Result<std::process::Child> {
        let strpath = filepath
            .to_str()
            .context("open with: can't parse filepath to str")?;
        let args = vec![program, strpath];
        self.open_with_args(args, use_term)
    }

    fn open_with_args(&self, args: Vec<&str>, use_term: bool) -> Result<std::process::Child> {
        if use_term {
            self.open_terminal(args)
        } else {
            self.open_directly(args)
        }
    }

    fn open_directly(&self, mut args: Vec<&str>) -> Result<std::process::Child> {
        if args.is_empty() {
            return Err(anyhow!("args shouldn't be empty"));
        }
        let executable = args.remove(0);
        execute_in_child(executable, &args)
    }

    // TODO: use terminal specific parameters instead of -e for all terminals
    fn open_terminal(&self, mut args: Vec<&str>) -> Result<std::process::Child> {
        args.insert(0, "-e");
        execute_in_child(&self.terminal, &args)
    }
}

/// Returns the opener created from opener file with the given terminal
/// application name.
/// It may fail if the file can't be read.
pub fn build_opener(path: &str, terminal: &str) -> Result<Opener> {
    let mut opener = Opener::new(terminal);
    let file = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let yaml = serde_yaml::from_reader(file)?;
    opener.update_from_file(&yaml);
    Ok(opener)
}
