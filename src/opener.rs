use std::collections::HashMap;
use std::error::Error;
use std::process::Command;

use serde_yaml;

#[derive(Clone, Hash, Eq, PartialEq)]
enum ExtensionKind {
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
struct OpenerAssociation {
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
struct OpenerInfo {
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
        let opener = yaml.get("opener");
        let use_term = yaml.get("use_term");
        if opener.is_some() && use_term.is_some() {
            Some(Self::new(
                opener.unwrap().as_str().unwrap(),
                use_term.unwrap().as_bool().unwrap(),
            ))
        } else {
            None
        }
    }
}

#[derive(Clone)]
pub struct Opener {
    terminal: String,
    opener_association: OpenerAssociation,
}

impl Opener {
    pub fn new(terminal: String) -> Self {
        Self {
            terminal,
            opener_association: OpenerAssociation::new(),
        }
    }

    pub fn open(&self, filepath: std::path::PathBuf) {
        if filepath.is_dir() {
            return;
        }

        if let Some(extension_os_str) = filepath.extension() {
            let extension = extension_os_str.to_str().unwrap();
            if let Some(open_info) = self.opener_association.opener_info(extension) {
                if open_info.use_term {
                    self.open_terminal(
                        open_info.opener.clone(),
                        filepath.to_str().unwrap().to_owned(),
                    )
                } else {
                    self.open_directly(
                        open_info.opener.clone(),
                        filepath.to_str().unwrap().to_owned(),
                    )
                }
            }
        }
    }

    pub fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        self.opener_association.update_from_file(yaml)
    }

    fn open_directly(&self, executable: String, filepath: String) {
        execute_in_child(&executable, &vec![&filepath]);
    }

    // TODO: use terminal specific parameters instead of -e for all terminals
    fn open_terminal(&self, executable: String, filepath: String) {
        execute_in_child(&self.terminal, &vec!["-e", &executable, &filepath]);
    }
}

/// Execute the command in a fork.
fn execute_in_child(exe: &str, args: &Vec<&str>) -> std::process::Child {
    eprintln!("exec exe {}, args {:?}", exe, args);
    Command::new(exe).args(args).spawn().unwrap()
}

pub fn load_opener(path: &str, terminal: String) -> Result<Opener, Box<dyn Error>> {
    let mut opener = Opener::new(terminal);
    let file = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(path).to_string()))?;
    let yaml = serde_yaml::from_reader(file)?;
    opener.update_from_file(&yaml);
    Ok(opener)
}
