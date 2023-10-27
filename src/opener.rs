use std::collections::HashMap;
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use log::info;
use serde_yaml;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::constant_strings_paths::{
    DEFAULT_AUDIO_OPENER, DEFAULT_IMAGE_OPENER, DEFAULT_OFFICE_OPENER, DEFAULT_OPENER,
    DEFAULT_READABLE_OPENER, DEFAULT_TEXT_OPENER, DEFAULT_VECTORIAL_OPENER, DEFAULT_VIDEO_OPENER,
};
use crate::decompress::{decompress_gz, decompress_xz, decompress_zip};
use crate::fileinfo::extract_extension;
use crate::log::write_log_line;

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
#[derive(Clone, Hash, Eq, PartialEq, Debug, Display, Default, EnumString, EnumIter)]
pub enum ExtensionKind {
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
                info!("extension kind iso");
                Self::Internal(InternalVariant::NotSupported)
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
                (
                    ExtensionKind::Vectorial,
                    OpenerInfo::external(DEFAULT_VECTORIAL_OPENER),
                ),
                (
                    ExtensionKind::Video,
                    OpenerInfo::external(DEFAULT_VIDEO_OPENER),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::DecompressZip),
                    OpenerInfo::internal(ExtensionKind::Internal(InternalVariant::DecompressZip))
                        .unwrap_or_default(),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::DecompressGz),
                    OpenerInfo::internal(ExtensionKind::Internal(InternalVariant::DecompressGz))
                        .unwrap_or_default(),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::DecompressXz),
                    OpenerInfo::internal(ExtensionKind::Internal(InternalVariant::DecompressXz))
                        .unwrap_or_default(),
                ),
                (
                    ExtensionKind::Internal(InternalVariant::NotSupported),
                    OpenerInfo::internal(ExtensionKind::Internal(InternalVariant::NotSupported))
                        .unwrap_or_default(),
                ),
                (ExtensionKind::Default, OpenerInfo::external(DEFAULT_OPENER)),
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
#[derive(Clone, Hash, PartialEq, Eq, Debug, Default)]
pub enum InternalVariant {
    #[default]
    DecompressZip,
    DecompressXz,
    DecompressGz,
    NotSupported,
}

/// A way to open one kind of files.
/// It's either an internal method or an external program.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct OpenerInfo {
    /// The external program used to open the file.
    pub external_program: Option<String>,
    /// The internal variant kind.
    pub internal_variant: Option<InternalVariant>,
    use_term: bool,
}

impl Default for OpenerInfo {
    fn default() -> Self {
        Self {
            external_program: Some("/usr/bin/xdg-open".to_owned()),
            internal_variant: None,
            use_term: false,
        }
    }
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

    fn internal(extension_kind: ExtensionKind) -> Result<Self> {
        match extension_kind {
            ExtensionKind::Internal(internal) => Ok(Self {
                external_program: None,
                internal_variant: Some(internal),
                use_term: false,
            }),
            _ => Err(anyhow!(
                "internal: unsupported extension_kind: {extension_kind:?}"
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

impl fmt::Display for OpenerInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
        let s = if let Some(external) = &self.external_program {
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

    /// Open multiple files.
    /// Files sharing an opener are opened in a single command ie.: `nvim a.txt b.rs c.py`.
    /// Only files opened with an external opener are supported.
    pub fn open_multiple(&self, file_paths: &[PathBuf]) -> Result<()> {
        let openers = self.regroup_openers(file_paths);
        for (open_info, file_paths) in openers.iter() {
            let file_paths_str = Self::collect_paths_as_str(file_paths);
            let mut args: Vec<&str> = vec![open_info.external_program.as_ref().unwrap()];
            args.extend(&file_paths_str);
            self.open_with_args(args, open_info.use_term)?;
        }
        Ok(())
    }

    /// Create an hashmap of openers -> [files].
    /// Each file in the collection share the same opener.
    fn regroup_openers(&self, file_paths: &[PathBuf]) -> HashMap<OpenerInfo, Vec<PathBuf>> {
        let mut openers: HashMap<OpenerInfo, Vec<PathBuf>> = HashMap::new();
        for file_path in file_paths.iter() {
            let open_info = self.get_opener(extract_extension(file_path));
            if open_info.external_program.is_some() {
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

    /// Open a file, using the configured method.
    /// It may fail if the program changed after reading the config file.
    /// It may also fail if the program can't handle this kind of files.
    /// This is quite a tricky method, there's many possible failures.
    pub fn open(&self, filepath: &Path) -> Result<()> {
        if filepath.is_dir() {
            return Err(anyhow!("open! can't execute a directory"));
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
                InternalVariant::DecompressZip => decompress_zip(filepath)?,
                InternalVariant::DecompressXz => decompress_xz(filepath)?,
                InternalVariant::DecompressGz => decompress_gz(filepath)?,
                InternalVariant::NotSupported => (),
            };
        }
        Ok(())
    }

    /// Returns the open info about this file.
    /// It's used to check if the file can be opened without specific actions or not.
    /// This opener can't mutate the status and can't ask for a sudo password.
    /// Some files requires root to be opened (ie. ISO files which are mounted).
    pub fn open_info(&self, filepath: &Path) -> &OpenerInfo {
        let extension = extract_extension(filepath);
        self.get_opener(extension)
    }

    /// Open a file with a given program.
    /// If the program requires a terminal, the terminal itself is opened
    /// and the program and its parameters are sent to it.
    pub fn open_with(
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

    fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        self.opener_association.update_from_file(yaml)
    }

    fn open_directly(&self, mut args: Vec<&str>) -> Result<std::process::Child> {
        let executable = args.remove(0);
        execute_in_child(executable, &args)
    }

    // TODO: use terminal specific parameters instead of -e for all terminals
    fn open_terminal(&self, mut args: Vec<&str>) -> Result<std::process::Child> {
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
pub fn execute_in_child<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<std::process::Child> {
    info!("execute_in_child. executable: {exe:?}, arguments: {args:?}");
    let log_line = format!("Execute: {exe:?}, arguments: {args:?}");
    write_log_line(log_line);
    Ok(Command::new(exe).args(args).spawn()?)
}

/// Execute a command with options in a fork.
/// Returns an handle to the child process.
/// Branch stdin, stderr and stdout to /dev/null
pub fn execute_in_child_without_output<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<std::process::Child> {
    info!("execute_in_child_without_output. executable: {exe:?}, arguments: {args:?}",);
    Ok(Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?)
}

pub fn execute_in_child_without_output_with_path<S, P>(
    exe: S,
    path: P,
    args: Option<&[&str]>,
) -> Result<std::process::Child>
where
    S: AsRef<std::ffi::OsStr> + fmt::Debug,
    P: AsRef<Path>,
{
    info!("execute_in_child_without_output_with_path. executable: {exe:?}, arguments: {args:?}");
    let params = args.unwrap_or(&[]);
    Ok(Command::new(exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(path)
        .args(params)
        .spawn()?)
}
/// Execute a command with options in a fork.
/// Wait for termination and return either :
/// `Ok(stdout)` if the status code is 0
/// an Error otherwise
/// Branch stdin and stderr to /dev/null
pub fn execute_and_capture_output<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<String> {
    info!("execute_and_capture_output. executable: {exe:?}, arguments: {args:?}",);
    let child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(anyhow!(
            "execute_and_capture_output: command didn't finish properly",
        ))
    }
}

/// Execute a command with options in a fork.
/// Wait for termination and return either `Ok(stdout)`.
/// Branch stdin and stderr to /dev/null
pub fn execute_and_capture_output_without_check<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<String> {
    info!("execute_and_capture_output_without_check. executable: {exe:?}, arguments: {args:?}",);
    let child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let output = child.wait_with_output()?;
    Ok(String::from_utf8(output.stdout)?)
}

/// Returns the opener created from opener file with the given terminal
/// application name.
/// It may fail if the file can't be read.
pub fn load_opener(path: &str, terminal: &str) -> Result<Opener> {
    let mut opener = Opener::new(terminal);
    let file = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let yaml = serde_yaml::from_reader(file)?;
    opener.update_from_file(&yaml);
    Ok(opener)
}
