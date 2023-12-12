use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde_yaml;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::common::is_program_in_path;
use crate::common::{
    OPENER_AUDIO, OPENER_DEFAULT, OPENER_IMAGE, OPENER_OFFICE, OPENER_PATH, OPENER_READABLE,
    OPENER_TEXT, OPENER_VECT, OPENER_VIDEO,
};
use crate::io::execute;
use crate::log_info;
use crate::modes::extract_extension;
use crate::modes::{decompress_gz, decompress_xz, decompress_zip};

/// Different kind of extensions for default openers.
#[derive(Clone, Hash, Eq, PartialEq, Debug, Display, Default, EnumString, EnumIter)]
enum Extension {
    #[default]
    Audio,
    Bitmap,
    Office,
    Readable,
    Text,
    Vectorial,
    Video,
    Zip,
    Gz,
    Xz,
    Iso,
    Default,
}

// TODO: move those associations to a config file
impl Extension {
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
            | "json" | "kt" | "lua" | "lock" | "log" | "md" | "micro" | "ninja" | "py" | "rkt"
            | "rs" | "scss" | "sh" | "srt" | "svelte" | "tex" | "toml" | "tsx" | "txt" | "vim"
            | "xml" | "yaml" | "yml" => Self::Text,

            "odt" | "odf" | "ods" | "odp" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => {
                Self::Office
            }

            "pdf" | "epub" => Self::Readable,

            "zip" => Self::Zip,

            "xz" | "7z" => Self::Xz,

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

    fn parse_yaml_file(path: &str) -> Option<serde_yaml::value::Value> {
        let Ok(file) =
            std::fs::File::open(std::path::Path::new(&shellexpand::tilde(path).to_string()))
        else {
            eprintln!("Couldn't find opener file at {path}. Using default.");
            log_info!("Unable to open {path}. Using default opener");
            return None;
        };
        let Ok(yaml) = serde_yaml::from_reader::<std::fs::File, serde_yaml::value::Value>(file)
        else {
            eprintln!("Couldn't read the opener config file at {path}.
See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/opener.yaml for an example. Using default.");
            log_info!("Unable to parse openers from {path}. Using default opener");
            return None;
        };
        Some(yaml)
    }

    fn update(&mut self, yaml: serde_yaml::value::Value) {
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
    NotSupported,
}

impl Internal {
    fn open(&self, path: &Path) -> Result<()> {
        match self {
            Self::Zip => decompress_zip(path),
            Self::Xz => decompress_xz(path),
            Self::Gz => decompress_gz(path),
            Self::NotSupported => Err(anyhow!("Can't be opened directly")),
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct External(String, bool);

impl External {
    fn new(opener_pair: (&str, bool)) -> Self {
        Self(opener_pair.0.to_owned(), opener_pair.1)
    }

    fn program(&self) -> &str {
        self.0.as_str()
    }

    fn use_term(&self) -> bool {
        self.1
    }

    fn open(&self, paths: &[&str], term: &str) -> Result<()> {
        let mut args: Vec<&str> = vec![self.program()];
        args.extend(paths);
        if self.use_term() {
            Self::with_term(args, term)?;
        } else {
            Self::without_term(args)?;
        }
        Ok(())
    }

    fn without_term(mut args: Vec<&str>) -> Result<std::process::Child> {
        if args.is_empty() {
            return Err(anyhow!("args shouldn't be empty"));
        }
        let executable = args.remove(0);
        execute(executable, &args)
    }

    // TODO: use terminal specific parameters instead of -e for all terminals
    fn with_term(mut args: Vec<&str>, term: &str) -> Result<std::process::Child> {
        args.insert(0, "-e");
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

    fn from_yaml(yaml: &serde_yaml::value::Value) -> Option<Self> {
        Some(Self::external((
            yaml.get("opener")?.as_str()?,
            yaml.get("use_term")?.as_bool()?,
        )))
    }

    fn is_external(&self) -> bool {
        matches!(self, Self::External(_))
    }

    fn is_valid(&self) -> bool {
        !self.is_external() || is_program_in_path(self.external_program().unwrap_or_default().0)
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
    pub association: Association,
}

impl Opener {
    /// Creates a new opener instance.
    /// Use the configured values from [`crate::common::OPENER_PATH`] if it can be parsed.
    pub fn new(terminal: &str) -> Self {
        Self {
            terminal: terminal.to_owned(),
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

    /// Open a file, using the configured method.
    /// It may fail if the program changed after reading the config file.
    /// It may also fail if the program can't handle this kind of files.
    /// This is quite a tricky method, there's many possible failures.
    pub fn open_single(&self, path: &Path) -> Result<()> {
        match self.kind(path) {
            Some(Kind::External(external)) => {
                external.open(&[path.to_str().context("couldn't")?], &self.terminal)
            }
            Some(Kind::Internal(internal)) => internal.open(path),
            None => Err(anyhow!("{p} can't be opened", p = path.display())),
        }
    }

    /// Open multiple files.
    /// Files sharing an opener are opened in a single command ie.: `nvim a.txt b.rs c.py`.
    /// Only files opened with an external opener are supported.
    pub fn open_multiple(&self, paths: &[PathBuf]) -> Result<()> {
        for (external, grouped_paths) in &self.regroup_per_opener(paths) {
            external.open(&Self::collect_paths_as_str(grouped_paths), &self.terminal)?;
        }
        Ok(())
    }

    /// Create an hashmap of openers -> [files].
    /// Each file in the collection share the same opener.
    fn regroup_per_opener(&self, paths: &[PathBuf]) -> HashMap<External, Vec<PathBuf>> {
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
}
