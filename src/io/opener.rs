use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde_yml::from_reader;
use serde_yml::Value;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::common::{
    is_in_path, tilde, OPENER_AUDIO, OPENER_DEFAULT, OPENER_IMAGE, OPENER_OFFICE, OPENER_PATH,
    OPENER_READABLE, OPENER_TEXT, OPENER_VECT, OPENER_VIDEO,
};
use crate::io::{execute, open_command_in_window};
use crate::log_info;
use crate::modes::{
    decompress_7z, decompress_gz, decompress_xz, decompress_zip, extract_extension,
};

/// Different kind of extensions for default openers.
#[derive(Clone, Hash, Eq, PartialEq, Debug, Display, Default, EnumString, EnumIter)]
pub enum Extension {
    #[default]
    Audio,
    Bitmap,
    Office,
    Readable,
    Text,
    Vectorial,
    Video,
    Zip,
    Sevenz,
    Gz,
    Xz,
    Iso,
    Default,
}

impl Extension {
    pub fn matcher(ext: &str) -> Self {
        match ext {
            "avif" | "bmp" | "gif" | "png" | "jpg" | "jpeg" | "pgm" | "ppm" | "webp" | "tiff" => {
                Self::Bitmap
            }

            "svg" => Self::Vectorial,

            "flac" | "m4a" | "wav" | "mp3" | "ogg" | "opus" => Self::Audio,

            "avi" | "mkv" | "av1" | "m4v" | "ts" | "webm" | "mov" | "wmv" => Self::Video,

            "build" | "c" | "cmake" | "conf" | "cpp" | "css" | "csv" | "cu" | "ebuild" | "eex"
            | "env" | "ex" | "exs" | "go" | "h" | "hpp" | "hs" | "html" | "ini" | "java" | "js"
            | "json" | "kt" | "lua" | "lock" | "log" | "md" | "micro" | "ninja" | "py" | "rkt"
            | "rs" | "scss" | "sh" | "srt" | "svelte" | "tex" | "toml" | "tsx" | "txt" | "vim"
            | "xml" | "yaml" | "yml" => Self::Text,

            "odt" | "odf" | "ods" | "odp" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => {
                Self::Office
            }

            "pdf" | "epub" => Self::Readable,

            "zip" => Self::Zip,

            "xz" => Self::Xz,

            "7z" | "7za" => Self::Sevenz,

            "lzip" | "lzma" | "rar" | "tgz" | "gz" | "bzip2" => Self::Gz,
            // iso files can't be mounted without more information than we hold in this enum :
            // we need to be able to change the status of the application to ask for a sudo password.
            // we can't use the "basic" opener to mount them.
            // ATM this is the only extension we can't open, it may change in the future.
            "iso" => {
                log_info!("extension kind iso");
                Self::Iso
            }
            _ => Self::Default,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Zip | Self::Xz | Self::Gz => "󰗄 ",
            Self::Readable => " ",
            Self::Iso => " ",
            Self::Text => " ",
            Self::Audio => " ",
            Self::Office => "󰈙 ",
            Self::Bitmap => " ",
            Self::Vectorial => "󰫨 ",
            Self::Video => " ",

            _ => " ",
        }
    }
}

macro_rules! open_file_with {
    ($self:ident, $key:expr, $variant:ident, $yaml:ident) => {
        if let Some(opener) = Kind::from_yaml(&$yaml[$key]) {
            $self
                .association
                .entry(Extension::$variant)
                .and_modify(|entry| *entry = opener);
        }
    };
}

/// Holds an association map between `Extension` and `Info`.
/// It's used to know how to open a kind of file.
#[derive(Clone)]
pub struct Association {
    association: HashMap<Extension, Kind>,
}

impl Default for Association {
    fn default() -> Self {
        Self {
            #[rustfmt::skip]
            association: HashMap::from([
                (Extension::Default,    Kind::external(OPENER_DEFAULT)),
                (Extension::Audio,      Kind::external(OPENER_AUDIO)),
                (Extension::Bitmap,     Kind::external(OPENER_IMAGE)),
                (Extension::Office,     Kind::external(OPENER_OFFICE)),
                (Extension::Readable,   Kind::external(OPENER_READABLE)),
                (Extension::Text,       Kind::external(OPENER_TEXT)),
                (Extension::Vectorial,  Kind::external(OPENER_VECT)),
                (Extension::Video,      Kind::external(OPENER_VIDEO)),
                (Extension::Sevenz,     Kind::Internal(Internal::Sevenz)),
                (Extension::Gz,         Kind::Internal(Internal::Gz)),
                (Extension::Xz,         Kind::Internal(Internal::Xz)),
                (Extension::Zip,        Kind::Internal(Internal::Zip)),
                (Extension::Iso,        Kind::Internal(Internal::NotSupported)),
            ]),
        }
    }
}

impl Association {
    fn with_config(mut self, path: &str) -> Self {
        let Some(yaml) = Self::parse_yaml_file(path) else {
            return self;
        };
        self.update(yaml);
        self.validate();
        log_info!("updated opener from {path}");
        self
    }

    fn parse_yaml_file(path: &str) -> Option<Value> {
        let Ok(file) = std::fs::File::open(std::path::Path::new(&tilde(path).to_string())) else {
            eprintln!("Couldn't find opener file at {path}. Using default.");
            log_info!("Unable to open {path}. Using default opener");
            return None;
        };
        let Ok(yaml) = from_reader::<std::fs::File, Value>(file) else {
            eprintln!("Couldn't read the opener config file at {path}.
See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/opener.yaml for an example. Using default.");
            log_info!("Unable to parse openers from {path}. Using default opener");
            return None;
        };
        Some(yaml)
    }

    fn update(&mut self, yaml: Value) {
        open_file_with!(self, "audio", Audio, yaml);
        open_file_with!(self, "bitmap_image", Bitmap, yaml);
        open_file_with!(self, "libreoffice", Office, yaml);
        open_file_with!(self, "readable", Readable, yaml);
        open_file_with!(self, "text", Text, yaml);
        open_file_with!(self, "default", Default, yaml);
        open_file_with!(self, "vectorial_image", Vectorial, yaml);
        open_file_with!(self, "video", Video, yaml);
    }

    fn validate(&mut self) {
        self.association.retain(|_, info| info.is_valid());
    }

    /// Converts itself into an hashmap of strings.
    /// Used to include openers in the help
    pub fn as_map_of_strings(&self) -> HashMap<String, String> {
        let mut associations: HashMap<String, String> = self
            .association
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        for s in Extension::iter() {
            let s = s.to_string();
            associations.entry(s).or_insert_with(|| "".to_owned());
        }
        associations
    }

    fn associate(&self, ext: &str) -> Option<&Kind> {
        self.association
            .get(&Extension::matcher(&ext.to_lowercase()))
    }
}

/// Some kind of files are "opened" using internal methods.
/// ATM only one kind of files is supported, compressed ones, which use
/// libarchive internally.
#[derive(Clone, Hash, PartialEq, Eq, Debug, Default)]
pub enum Internal {
    #[default]
    Zip,
    Xz,
    Gz,
    Sevenz,
    NotSupported,
}

impl Internal {
    fn open(&self, path: &Path) -> Result<()> {
        match self {
            Self::Sevenz => decompress_7z(path),
            Self::Zip => decompress_zip(path),
            Self::Xz => decompress_xz(path),
            Self::Gz => decompress_gz(path),
            Self::NotSupported => Err(anyhow!("Can't be opened directly")),
        }
    }
}

/// Used to open file externally (with other programs).
/// Most of the files are "opened" this way, only archives which could be
/// decompressed interally aren't.
///
/// It holds a path to the file (as a string, for convernience) and a
/// flag set to true if the file is opened in a terminal.
/// - without a terminal, the file is opened by its application,
/// - with a terminal, it starts a new terminal (from configuration) and then the program.
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct External(String, bool);

impl External {
    fn new(opener_pair: (&str, bool)) -> Self {
        Self(opener_pair.0.to_owned(), opener_pair.1)
    }

    fn program(&self) -> &str {
        self.0.as_str()
    }

    pub fn use_term(&self) -> bool {
        self.1
    }

    fn open(&self, paths: &[&str], term: &str, term_flag: &str) -> Result<()> {
        let mut args: Vec<&str> = vec![self.program()];
        args.extend(paths);
        if self.use_term() {
            Self::with_term(args, term, term_flag)?;
        } else {
            Self::without_term(args)?;
        }
        Ok(())
    }

    fn open_in_window<'a>(&'a self, path: &'a str) -> Result<()> {
        let arg = format!("{program} {path}", program = self.program(),);
        open_command_in_window(&arg)
    }

    fn open_multiple_in_window(&self, paths: &[PathBuf]) -> Result<()> {
        let arg = paths
            .iter()
            .filter_map(|p| p.to_str())
            .collect::<Vec<_>>()
            .join(" ");
        open_command_in_window(&format!("{program} {arg}", program = self.program()))
    }

    fn without_term(mut args: Vec<&str>) -> Result<std::process::Child> {
        if args.is_empty() {
            return Err(anyhow!("args shouldn't be empty"));
        }
        let executable = args.remove(0);
        execute(executable, &args)
    }

    fn with_term<'a>(
        mut args: Vec<&'a str>,
        term: &'a str,
        term_flag: &'a str,
    ) -> Result<std::process::Child> {
        args.insert(0, term_flag);
        execute(term, &args)
    }
}

/// A way to open one kind of files.
/// It's either an internal method or an external program.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Kind {
    Internal(Internal),
    External(External),
}

impl Default for Kind {
    fn default() -> Self {
        Self::external(OPENER_DEFAULT)
    }
}

impl Kind {
    fn external(opener_pair: (&str, bool)) -> Self {
        Self::External(External::new(opener_pair))
    }

    fn from_yaml(yaml: &Value) -> Option<Self> {
        Some(Self::external((
            yaml.get("opener")?.as_str()?,
            yaml.get("use_term")?.as_bool()?,
        )))
    }

    fn is_external(&self) -> bool {
        matches!(self, Self::External(_))
    }

    fn is_valid(&self) -> bool {
        !self.is_external() || is_in_path(self.external_program().unwrap_or_default().0)
    }

    fn external_program(&self) -> Result<(&str, bool)> {
        let Self::External(External(program, use_term)) = self else {
            return Err(anyhow!("not an external opener"));
        };
        Ok((program, *use_term))
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
        let s = if let Self::External(External(program, _)) = &self {
            program
        } else {
            "internal"
        };
        write!(f, "{s}")
    }
}

/// Basic file opener.
///
/// Holds the associations between different kind of files and opener method
/// as well as the name of the terminal configured by the user.
/// It's also responsible for "opening" most kind of files.
/// There's two exceptions :
/// - iso files, which are mounted. It requires a sudo password.
/// - neovim filepicking. It uses a socket to send RPC command.
///
/// It may open a single or multiple files, trying to regroup them by opener.
#[derive(Clone)]
pub struct Opener {
    /// The name of the configured terminal application
    pub terminal: String,
    /// Terminal flag used to run a command at startup. Usually (but not always) -e.
    /// See the default config file for more information.
    pub terminal_flag: String,
    /// The association of openers for every kind of files
    pub association: Association,
}

impl Opener {
    /// Creates a new opener instance.
    /// Use the configured values from [`crate::common::OPENER_PATH`] if it can be parsed.
    pub fn new(terminal: &str, terminal_flag: &str) -> Self {
        Self {
            terminal: terminal.to_owned(),
            terminal_flag: terminal_flag.to_owned(),
            association: Association::default().with_config(OPENER_PATH),
        }
    }

    /// Returns the open info about this file.
    /// It's used to check if the file can be opened without specific actions or not.
    /// This opener can't mutate the status and can't ask for a sudo password.
    /// Some files requires root to be opened (ie. ISO files which are mounted).
    pub fn kind(&self, path: &Path) -> Option<&Kind> {
        if path.is_dir() {
            return None;
        }
        self.association.associate(extract_extension(path))
    }

    /// Does this extension requires a terminal ?
    pub fn extension_use_term(&self, extension: &str) -> bool {
        if let Some(Kind::External(external)) = self.association.associate(extension) {
            external.use_term()
        } else {
            false
        }
    }

    pub fn use_term(&self, path: &Path) -> bool {
        match self.kind(path) {
            None => false,
            Some(Kind::Internal(_)) => false,
            Some(Kind::External(external)) => external.use_term(),
        }
    }

    /// Open a file, using the configured method.
    /// It may fail if the program changed after reading the config file.
    /// It may also fail if the program can't handle this kind of files.
    /// This is quite a tricky method, there's many possible failures.
    pub fn open_single(&self, path: &Path) -> Result<()> {
        match self.kind(path) {
            Some(Kind::External(external)) => external.open(
                &[path.to_str().context("couldn't")?],
                &self.terminal,
                &self.terminal_flag,
            ),
            Some(Kind::Internal(internal)) => internal.open(path),
            None => Err(anyhow!("{p} can't be opened", p = path.display())),
        }
    }

    /// Open multiple files.
    /// Files sharing an opener are opened in a single command ie.: `nvim a.txt b.rs c.py`.
    /// Only files opened with an external opener are supported.
    pub fn open_multiple(&self, openers: HashMap<External, Vec<PathBuf>>) -> Result<()> {
        for (external, grouped_paths) in openers.iter() {
            let _ = external.open(
                &Self::collect_paths_as_str(grouped_paths),
                &self.terminal,
                &self.terminal_flag,
            );
        }
        Ok(())
    }

    /// Create an hashmap of openers -> `[files]`.
    /// Each file in the collection share the same opener.
    pub fn regroup_per_opener(&self, paths: &[PathBuf]) -> HashMap<External, Vec<PathBuf>> {
        let mut openers: HashMap<External, Vec<PathBuf>> = HashMap::new();
        for path in paths {
            let Some(Kind::External(pair)) = self.kind(path) else {
                continue;
            };
            openers
                .entry(External(pair.0.to_owned(), pair.1).to_owned())
                .and_modify(|files| files.push((*path).to_owned()))
                .or_insert(vec![(*path).to_owned()]);
        }
        openers
    }

    /// Convert a slice of `PathBuf` into their string representation.
    /// Files which are directory are skipped.
    fn collect_paths_as_str(paths: &[PathBuf]) -> Vec<&str> {
        paths
            .iter()
            .filter(|fp| !fp.is_dir())
            .filter_map(|fp| fp.to_str())
            .collect()
    }

    pub fn open_in_window(&self, path: &Path) {
        let Some(Kind::External(external)) = self.kind(path) else {
            return;
        };
        if !external.use_term() {
            return;
        };
        let _ = external.open_in_window(path.to_string_lossy().as_ref());
    }

    pub fn open_multiple_in_window(&self, openers: HashMap<External, Vec<PathBuf>>) -> Result<()> {
        let (external, paths) = openers.iter().next().unwrap();
        external.open_multiple_in_window(paths)
    }
}
