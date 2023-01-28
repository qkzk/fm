use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use log::info;
use serde_yaml;

use crate::compress::decompress;
use crate::constant_strings_paths::{
    DEFAULT_AUDIO_OPENER, DEFAULT_IMAGE_OPENER, DEFAULT_OFFICE_OPENER, DEFAULT_OPENER,
    DEFAULT_READABLE_OPENER, DEFAULT_TEXT_OPENER, DEFAULT_VECTORIAL_OPENER, DEFAULT_VIDEO_OPENER,
};
use crate::fileinfo::extract_extension;
use crate::fm_error::{FmError, FmResult};

fn find_it<P>(exe_name: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let full_path = dir.join(&exe_name);
            if full_path.is_file() {
                Some(full_path)
            } else {
                None
            }
        })
    })
}

/// Different kind of extensions for default openers.
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub enum ExtensionKind {
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
    fn parse(ext: &str) -> Self {
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

            "lzip" | "lzma" | "rar" | "tgz" | "zip" | "gzip" | "bzip2" | "xz" | "7z" => {
                Self::Internal(InternalVariant::Decompress)
            }

            _ => Self::Default,
        }
    }
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
                    OpenerInfo::external(DEFAULT_AUDIO_OPENER),
                ),
                (
                    ExtensionKind::Bitmap,
                    OpenerInfo::external(DEFAULT_IMAGE_OPENER),
                ),
                (
                    ExtensionKind::Office,
                    OpenerInfo::external(DEFAULT_OFFICE_OPENER),
                ),
                (
                    ExtensionKind::Readable,
                    OpenerInfo::external(DEFAULT_READABLE_OPENER),
                ),
                (
                    ExtensionKind::Text,
                    OpenerInfo::external(DEFAULT_TEXT_OPENER),
                ),
                (ExtensionKind::Default, OpenerInfo::external(DEFAULT_OPENER)),
                (
                    ExtensionKind::Vectorial,
                    OpenerInfo::external(DEFAULT_VECTORIAL_OPENER),
                ),
                (
                    ExtensionKind::Video,
                    OpenerInfo::external(DEFAULT_VIDEO_OPENER),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::Decompress),
                    OpenerInfo::internal(ExtensionKind::Internal(InternalVariant::Decompress))
                        .unwrap(),
                ),
            ]),
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

impl OpenerAssociation {
    fn opener_info(&self, ext: &str) -> Option<&OpenerInfo> {
        self.association.get(&ExtensionKind::parse(ext))
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
        info!("update from file");
    }

    fn validate_openers(&mut self) {
        self.association.retain(|_, opener| {
            opener.external_program.is_none()
                || find_it(opener.external_program.as_ref().unwrap()).is_some()
        });
    }
}

/// Some kind of files are "opened" using internal methods.
/// ATM only one kind of files is supported, compressed ones, which use
/// libarchive internally.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub enum InternalVariant {
    Decompress,
}

/// A way to open one kind of files.
/// It's either an internal method or an external program.
#[derive(Clone, Debug)]
pub struct OpenerInfo {
    /// The external program used to open the file.
    pub external_program: Option<String>,
    /// The internal variant kind.
    pub internal_variant: Option<InternalVariant>,
    use_term: bool,
}

impl OpenerInfo {
    fn external(opener_pair: (&str, bool)) -> Self {
        let opener = opener_pair.0;
        let use_term = opener_pair.1;
        Self {
            external_program: Some(opener.to_owned()),
            internal_variant: None,
            use_term,
        }
    }

    fn internal(extension_kind: ExtensionKind) -> FmResult<Self> {
        match extension_kind {
            ExtensionKind::Internal(internal) => Ok(Self {
                external_program: None,
                internal_variant: Some(internal),
                use_term: false,
            }),
            _ => Err(FmError::custom(
                "internal",
                &format!("unsupported extension_kind: {extension_kind:?}"),
            )),
        }
    }

    fn from_yaml(yaml: &serde_yaml::value::Value) -> Option<Self> {
        Some(Self::external((
            yaml.get("opener")?.as_str()?,
            yaml.get("use_term")?.as_bool()?,
        )))
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
    default_opener: OpenerInfo,
}

impl Opener {
    /// Creates a new opener instance.
    /// Default values are used. It may be uptaded with configured ones later.
    pub fn new(terminal: &str) -> Self {
        Self {
            terminal: terminal.to_owned(),
            opener_association: OpenerAssociation::new(),
            default_opener: OpenerInfo::external(DEFAULT_OPENER),
        }
    }

    fn get_opener(&self, extension: &str) -> &OpenerInfo {
        if let Some(opener) = self.opener_association.opener_info(extension) {
            opener
        } else {
            &self.default_opener
        }
    }

    /// Open a file, using the configured method.
    /// It may fail if the program changed after reading the config file.
    /// It may also fail if the program can't handle this kind of files.
    /// This is quite a tricky method, there's many possible failures.
    pub fn open(&self, filepath: &Path) -> FmResult<()> {
        if filepath.is_dir() {
            return Err(FmError::custom("open", "Can't execute a directory"));
        }
        let extension = extract_extension(filepath);
        let open_info = self.get_opener(extension);
        if open_info.external_program.is_some() {
            self.open_with(
                open_info.external_program.as_ref().unwrap(),
                open_info.use_term,
                filepath,
            )?;
        } else {
            match open_info.internal_variant.as_ref().unwrap() {
                InternalVariant::Decompress => decompress(filepath)?,
            };
        }
        Ok(())
    }

    /// Open a file with a given program.
    /// If the program requires a terminal, the terminal itself is opened
    /// and the program and its parameters are sent to it.
    pub fn open_with(
        &self,
        program: &str,
        use_term: bool,
        filepath: &std::path::Path,
    ) -> FmResult<std::process::Child> {
        let strpath = filepath
            .to_str()
            .ok_or_else(|| FmError::custom("open with", "Can't parse filepath to str"))?;
        let args = vec![program, strpath];
        if use_term {
            self.open_terminal(args)
        } else {
            self.open_directly(args)
        }
    }

    fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        self.opener_association.update_from_file(yaml)
    }

    fn open_directly(&self, mut args: Vec<&str>) -> FmResult<std::process::Child> {
        let executable = args.remove(0);
        execute_in_child(executable, &args)
    }

    // TODO: use terminal specific parameters instead of -e for all terminals
    fn open_terminal(&self, mut args: Vec<&str>) -> FmResult<std::process::Child> {
        args.insert(0, "-e");
        execute_in_child(&self.terminal, &args)
    }

    /// Returns the Opener association associated to a kind of file.
    pub fn get(&self, kind: ExtensionKind) -> Option<&OpenerInfo> {
        self.opener_association.association.get(&kind)
    }
}

/// Execute a command with options in a fork.
/// Returns an handle to the child process.
pub fn execute_in_child(exe: &str, args: &Vec<&str>) -> FmResult<std::process::Child> {
    info!(
        "execute_in_child. executable: {}, arguments: {:?}",
        exe, args
    );
    Ok(Command::new(exe).args(args).spawn()?)
}

/// Returns the opener created from opener file with the given terminal
/// application name.
/// It may fail if the file can't be read.
pub fn load_opener(path: &str, terminal: &str) -> FmResult<Opener> {
    let mut opener = Opener::new(terminal);
    let file = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let yaml = serde_yaml::from_reader(file)?;
    opener.update_from_file(&yaml);
    Ok(opener)
}
