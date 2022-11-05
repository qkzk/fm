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
            "avif" => Self::Bitmap,
            "bmp" => Self::Bitmap,
            "gif" => Self::Bitmap,
            "png" => Self::Bitmap,
            "jpg" => Self::Bitmap,
            "jpeg" => Self::Bitmap,
            "pgm" => Self::Bitmap,
            "ppm" => Self::Bitmap,
            "webp" => Self::Bitmap,
            "tiff" => Self::Bitmap,

            "svg" => Self::Vectorial,

            "flac" => Self::Audio,
            "m4a" => Self::Audio,
            "wav" => Self::Audio,
            "mp3" => Self::Audio,
            "ogg" => Self::Audio,
            "opus" => Self::Audio,

            "avi" => Self::Video,
            "mkv" => Self::Video,
            "av1" => Self::Video,
            "m4v" => Self::Video,
            "ts" => Self::Video,
            "webm" => Self::Video,
            "mov" => Self::Video,
            "wmv" => Self::Video,

            "build" => Self::Text,
            "c" => Self::Text,
            "cmake" => Self::Text,
            "conf" => Self::Text,
            "cpp" => Self::Text,
            "css" => Self::Text,
            "csv" => Self::Text,
            "cu" => Self::Text,
            "ebuild" => Self::Text,
            "eex" => Self::Text,
            "env" => Self::Text,
            "ex" => Self::Text,
            "exs" => Self::Text,
            "go" => Self::Text,
            "h" => Self::Text,
            "hpp" => Self::Text,
            "hs" => Self::Text,
            "html" => Self::Text,
            "ini" => Self::Text,
            "java" => Self::Text,
            "js" => Self::Text,
            "json" => Self::Text,
            "kt" => Self::Text,
            "lua" => Self::Text,
            "log" => Self::Text,
            "md" => Self::Text,
            "micro" => Self::Text,
            "ninja" => Self::Text,
            "py" => Self::Text,
            "rkt" => Self::Text,
            "rs" => Self::Text,
            "scss" => Self::Text,
            "sh" => Self::Text,
            "srt" => Self::Text,
            "svelte" => Self::Text,
            "tex" => Self::Text,
            "toml" => Self::Text,
            "tsx" => Self::Text,
            "txt" => Self::Text,
            "vim" => Self::Text,
            "xml" => Self::Text,
            "yaml" => Self::Text,
            "yml" => Self::Text,

            "odt" => Self::Office,
            "odf" => Self::Office,
            "ods" => Self::Office,
            "odp" => Self::Office,
            "doc" => Self::Office,
            "docx" => Self::Office,
            "xls" => Self::Office,
            "xlsx" => Self::Office,
            "ppt" => Self::Office,
            "pptx" => Self::Office,

            "pdf" => Self::Readable,

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
        let mut association = HashMap::new();

        association.insert(ExtensionKind::Audio, OpenerInfo::new("moc", true));
        association.insert(ExtensionKind::Bitmap, OpenerInfo::new("viewnior", false));
        association.insert(ExtensionKind::Office, OpenerInfo::new("libreoffice", false));
        association.insert(ExtensionKind::Readable, OpenerInfo::new("zathura", false));
        association.insert(ExtensionKind::Text, OpenerInfo::new("nvim", true));
        association.insert(ExtensionKind::Unknown, OpenerInfo::new("xdg-open", false));
        association.insert(ExtensionKind::Vectorial, OpenerInfo::new("inkscape", false));
        association.insert(ExtensionKind::Video, OpenerInfo::new("mpv", false));
        Self { association }
    }

    fn opener_info(&self, ext: &str) -> Option<&OpenerInfo> {
        self.association.get(&ExtensionKind::parse(ext))
    }

    fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        if let Some(audio) = OpenerInfo::from_yaml(&yaml["audio"]) {
            self.association
                .entry(ExtensionKind::Audio)
                .and_modify(|e| *e = audio);
        }
        if let Some(bitmap_image) = OpenerInfo::from_yaml(&yaml["bitmap_image"]) {
            self.association
                .entry(ExtensionKind::Bitmap)
                .and_modify(|e| *e = bitmap_image);
        }
        if let Some(libreoffice) = OpenerInfo::from_yaml(&yaml["libreoffice"]) {
            self.association
                .entry(ExtensionKind::Office)
                .and_modify(|e| *e = libreoffice);
        }
        if let Some(readable) = OpenerInfo::from_yaml(&yaml["readable"]) {
            self.association
                .entry(ExtensionKind::Readable)
                .and_modify(|e| *e = readable);
        }
        if let Some(text) = OpenerInfo::from_yaml(&yaml["text"]) {
            self.association
                .entry(ExtensionKind::Text)
                .and_modify(|e| *e = text);
        }
        if let Some(unknown) = OpenerInfo::from_yaml(&yaml["unknown"]) {
            self.association
                .entry(ExtensionKind::Unknown)
                .and_modify(|e| *e = unknown);
        }
        if let Some(vectorial_image) = OpenerInfo::from_yaml(&yaml["vectorial_image"]) {
            self.association
                .entry(ExtensionKind::Vectorial)
                .and_modify(|e| *e = vectorial_image);
        }
        if let Some(video) = OpenerInfo::from_yaml(&yaml["video"]) {
            self.association
                .entry(ExtensionKind::Video)
                .and_modify(|e| *e = video);
        }
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
