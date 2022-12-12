use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use log::info;
use serde_yaml;

use crate::fm_error::{ErrorVariant, FmError, FmResult};

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
    Compressed(String),
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

            "tgz" | "zip" | "gzip" | "bzip2" | "xz" | "7z" => Self::Compressed(ext.to_owned()),

            _ => Self::Default,
        }
    }
}

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
                    OpenerInfo::new(vec!["mocp".to_owned()], true),
                ),
                (
                    ExtensionKind::Bitmap,
                    OpenerInfo::new(vec!["viewnior".to_owned()], false),
                ),
                (
                    ExtensionKind::Office,
                    OpenerInfo::new(vec!["libreoffice".to_owned()], false),
                ),
                (
                    ExtensionKind::Readable,
                    OpenerInfo::new(vec!["zathura".to_owned()], false),
                ),
                (
                    ExtensionKind::Text,
                    OpenerInfo::new(vec!["nvim".to_owned()], true),
                ),
                (
                    ExtensionKind::Default,
                    OpenerInfo::new(vec!["xdg-open".to_owned()], false),
                ),
                (
                    ExtensionKind::Vectorial,
                    OpenerInfo::new(vec!["inkscape".to_owned()], false),
                ),
                (
                    ExtensionKind::Video,
                    OpenerInfo::new(vec!["mpv".to_owned()], false),
                ),
                (
                    ExtensionKind::Compressed("tgz".to_owned()),
                    OpenerInfo::new(vec!["tar".to_owned(), "xf".to_owned()], true),
                ),
                (
                    ExtensionKind::Compressed("zip".to_owned()),
                    OpenerInfo::new(vec!["unzip".to_owned()], true),
                ),
                (
                    ExtensionKind::Compressed("gzip".to_owned()),
                    OpenerInfo::new(vec!["gunzip".to_owned()], true),
                ),
                (
                    ExtensionKind::Compressed("bzip2".to_owned()),
                    OpenerInfo::new(vec!["bunzip2".to_owned()], true),
                ),
                (
                    ExtensionKind::Compressed("xz".to_owned()),
                    OpenerInfo::new(vec!["xz".to_owned(), "-d".to_owned()], true),
                ),
                (
                    ExtensionKind::Compressed("7z".to_owned()),
                    OpenerInfo::new(vec!["7z".to_owned(), "e".to_owned()], true),
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
    }

    fn validate_openers(&mut self) {
        self.association
            .retain(|_, opener| find_it(opener.opener[0].clone()).is_some())
    }
}

#[derive(Clone, Debug)]
pub struct OpenerInfo {
    pub opener: Vec<String>,
    use_term: bool,
}

impl OpenerInfo {
    fn new(opener: Vec<String>, use_term: bool) -> Self {
        Self { opener, use_term }
    }

    fn from_yaml(yaml: &serde_yaml::value::Value) -> Option<Self> {
        Some(Self::new(
            yaml.get("opener")?
                .as_str()?
                .split(" ")
                .map(|s| s.to_owned())
                .collect(),
            yaml.get("use_term")?.as_bool()?,
        ))
    }
}

#[derive(Clone)]
pub struct Opener {
    pub terminal: String,
    pub opener_association: OpenerAssociation,
    default_opener: OpenerInfo,
}

impl Opener {
    pub fn new(terminal: String) -> Self {
        Self {
            terminal,
            opener_association: OpenerAssociation::new(),
            default_opener: OpenerInfo::new(vec!["xdg-open".to_owned()], false),
        }
    }

    fn get_opener(&self, extension: &str) -> &OpenerInfo {
        if let Some(opener) = self.opener_association.opener_info(extension) {
            opener
        } else {
            &self.default_opener
        }
    }

    pub fn open(&self, filepath: std::path::PathBuf) -> FmResult<std::process::Child> {
        if filepath.is_dir() {
            return Err(FmError::new(
                ErrorVariant::CUSTOM("open".to_owned()),
                "Can't execute a directory",
            ));
        }

        let extension_os_string = filepath
            .extension()
            .ok_or_else(|| {
                FmError::new(
                    ErrorVariant::CUSTOM("open".to_owned()),
                    "Unreadable extension",
                )
            })?
            .to_owned();
        let extension = extension_os_string.to_str().ok_or_else(|| {
            FmError::new(
                ErrorVariant::CUSTOM("open".to_owned()),
                "Extension couldn't be parsed correctly",
            )
        })?;
        self.open_with(self.get_opener(extension), filepath)
    }

    pub fn open_with(
        &self,
        open_info: &OpenerInfo,
        filepath: std::path::PathBuf,
    ) -> FmResult<std::process::Child> {
        let mut args = open_info.opener.clone();
        args.push(filepath.as_os_str().to_owned().into_string()?);
        if open_info.use_term {
            self.open_terminal(args)
        } else {
            self.open_directly(args)
        }
    }

    pub fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        self.opener_association.update_from_file(yaml)
    }

    fn open_directly(&self, mut args: Vec<String>) -> FmResult<std::process::Child> {
        let executable = args.remove(0);
        execute_in_child(&executable, &args.iter().map(|s| &**s).collect())
    }

    // TODO: use terminal specific parameters instead of -e for all terminals
    fn open_terminal(&self, mut args: Vec<String>) -> FmResult<std::process::Child> {
        args.insert(0, "-e".to_owned());
        execute_in_child(&self.terminal, &args.iter().map(|s| &**s).collect())
    }

    pub fn get(&self, kind: ExtensionKind) -> Option<&OpenerInfo> {
        self.opener_association.association.get(&kind)
    }

    pub fn open_terminal_with_args(&self, args: Vec<&str>) -> FmResult<std::process::Child> {
        execute_in_child(&self.terminal, &args)
    }
}

/// Execute the command in a fork.
pub fn execute_in_child(exe: &str, args: &Vec<&str>) -> FmResult<std::process::Child> {
    info!(
        "execute_in_child. executable: {}, arguments: {:?}",
        exe, args
    );
    Ok(Command::new(exe).args(args).spawn()?)
}

pub fn execute_in_child_piped(exe: &str, args: &Vec<&str>) -> FmResult<std::process::Child> {
    info!(
        "execute_in_child. executable: {}, arguments: {:?}",
        exe, args
    );
    Ok(Command::new(exe)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?)
}

pub fn load_opener(path: &str, terminal: String) -> Result<Opener, Box<dyn Error>> {
    let mut opener = Opener::new(terminal);
    let file = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let yaml = serde_yaml::from_reader(file)?;
    opener.update_from_file(&yaml);
    Ok(opener)
}
