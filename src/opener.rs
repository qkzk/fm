use std::collections::HashMap;
use std::error::Error;
use std::process::Command;

use log::info;
use serde_yaml;

use crate::fm_error::{FmError, FmResult};

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum ExtensionKind {
    Audio,
    Bitmap,
    Office,
    Readable,
    Text,
    Unknown,
    Vectorial,
    Video,
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

            _ => Self::Unknown,
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
                (ExtensionKind::Audio, OpenerInfo::new("moc", true)),
                (ExtensionKind::Bitmap, OpenerInfo::new("viewnior", false)),
                (ExtensionKind::Office, OpenerInfo::new("libreoffice", false)),
                (ExtensionKind::Readable, OpenerInfo::new("zathura", false)),
                (ExtensionKind::Text, OpenerInfo::new("nvim", true)),
                (ExtensionKind::Unknown, OpenerInfo::new("xdg-open", false)),
                (ExtensionKind::Vectorial, OpenerInfo::new("inkscape", false)),
                (ExtensionKind::Video, OpenerInfo::new("mpv", false)),
            ]),
        }
    }
}

macro_rules! open_file_with {
    ($self:ident, $key:ident, $variant:ident, $yaml:ident) => {
        if let Some($key) = OpenerInfo::from_yaml(&$yaml["$key"]) {
            $self
                .association
                .entry(ExtensionKind::$variant)
                .and_modify(|e| *e = $key);
        }
    };
}
impl OpenerAssociation {
    fn opener_info(&self, ext: &str) -> Option<&OpenerInfo> {
        self.association.get(&ExtensionKind::parse(ext))
    }

    fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        open_file_with!(self, audio, Audio, yaml);
        open_file_with!(self, bitmap_image, Bitmap, yaml);
        open_file_with!(self, libreoffice, Office, yaml);
        open_file_with!(self, readable, Readable, yaml);
        open_file_with!(self, text, Text, yaml);
        open_file_with!(self, unknown, Unknown, yaml);
        open_file_with!(self, vectorial_image, Vectorial, yaml);
        open_file_with!(self, video, Video, yaml);
    }
}

#[derive(Clone)]
pub struct OpenerInfo {
    opener: String,
    use_term: bool,
}

impl OpenerInfo {
    fn new(opener: &str, use_term: bool) -> Self {
        Self {
            opener: opener.to_owned(),
            use_term,
        }
    }

    fn from_yaml(yaml: &serde_yaml::value::Value) -> Option<Self> {
        Some(Self::new(
            yaml.get("opener")?.as_str()?,
            yaml.get("use_term")?.as_bool()?,
        ))
    }
}

#[derive(Clone)]
pub struct Opener {
    terminal: String,
    pub opener_association: OpenerAssociation,
}

impl Opener {
    pub fn new(terminal: String) -> Self {
        Self {
            terminal,
            opener_association: OpenerAssociation::new(),
        }
    }

    pub fn open(&self, filepath: std::path::PathBuf) -> FmResult<std::process::Child> {
        if filepath.is_dir() {
            return Err(FmError::new("Can't execute a directory"));
        }

        let extension_os_string = filepath
            .extension()
            .ok_or_else(|| FmError::new("Unreadable extension"))?
            .to_owned();
        let extension = extension_os_string
            .to_str()
            .ok_or_else(|| FmError::new("Extension couldn't be parsed correctly"))?;
        self.open_with(
            self.opener_association
                .opener_info(extension)
                .ok_or_else(|| FmError::new("This extension has no known opener"))?,
            filepath,
        )
    }

    pub fn open_with(
        &self,
        open_info: &OpenerInfo,
        filepath: std::path::PathBuf,
    ) -> FmResult<std::process::Child> {
        if open_info.use_term {
            self.open_terminal(
                open_info.opener.clone(),
                filepath.as_os_str().to_owned().into_string()?,
            )
        } else {
            self.open_directly(
                open_info.opener.clone(),
                filepath.as_os_str().to_owned().into_string()?,
            )
        }
    }

    pub fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        self.opener_association.update_from_file(yaml)
    }

    fn open_directly(&self, executable: String, filepath: String) -> FmResult<std::process::Child> {
        execute_in_child(&executable, &vec![&filepath])
    }

    // TODO: use terminal specific parameters instead of -e for all terminals
    fn open_terminal(&self, executable: String, filepath: String) -> FmResult<std::process::Child> {
        execute_in_child(&self.terminal, &vec!["-e", &executable, &filepath])
    }

    pub fn get(&self, kind: ExtensionKind) -> Option<&OpenerInfo> {
        self.opener_association.association.get(&kind)
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

pub fn load_opener(path: &str, terminal: String) -> Result<Opener, Box<dyn Error>> {
    let mut opener = Opener::new(terminal);
    let file = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let yaml = serde_yaml::from_reader(file)?;
    opener.update_from_file(&yaml);
    Ok(opener)
}
