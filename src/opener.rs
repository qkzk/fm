use std::collections::HashMap;
use std::error::Error;
use std::process::Command;

use serde_yaml;

#[derive(Clone, Hash, Eq, PartialEq)]
enum ExtensionCategory {
    Audio,
    BitmapImage,
    LibreOffice,
    Readable,
    Text,
    Unknown,
    VectorialImage,
    Video,
}

// TODO: move those associations to a config file
impl ExtensionCategory {
    fn parse(ext: &str) -> Self {
        match ext {
            "avif" => Self::BitmapImage,
            "bmp" => Self::BitmapImage,
            "gif" => Self::BitmapImage,
            "png" => Self::BitmapImage,
            "jpg" => Self::BitmapImage,
            "jpeg" => Self::BitmapImage,
            "pgm" => Self::BitmapImage,
            "ppm" => Self::BitmapImage,
            "webp" => Self::BitmapImage,
            "tiff" => Self::BitmapImage,

            "svg" => Self::VectorialImage,

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

            "odt" => Self::LibreOffice,
            "odf" => Self::LibreOffice,
            "ods" => Self::LibreOffice,
            "odp" => Self::LibreOffice,
            "doc" => Self::LibreOffice,
            "docx" => Self::LibreOffice,
            "xls" => Self::LibreOffice,
            "xlsx" => Self::LibreOffice,
            "ppt" => Self::LibreOffice,
            "pptx" => Self::LibreOffice,

            "pdf" => Self::Readable,

            _ => Self::Unknown,
        }
    }
}

#[derive(Clone)]
struct OpenerAssociation {
    association: HashMap<ExtensionCategory, OpenerInfo>,
}

impl OpenerAssociation {
    fn new() -> Self {
        let mut association = HashMap::new();

        association.insert(ExtensionCategory::Audio, OpenerInfo::new("moc", true));
        association.insert(
            ExtensionCategory::BitmapImage,
            OpenerInfo::new("viewnior", false),
        );
        association.insert(
            ExtensionCategory::LibreOffice,
            OpenerInfo::new("libreoffice", false),
        );
        association.insert(
            ExtensionCategory::Readable,
            OpenerInfo::new("zathura", false),
        );
        association.insert(ExtensionCategory::Text, OpenerInfo::new("nvim", true));
        association.insert(
            ExtensionCategory::Unknown,
            OpenerInfo::new("xdg-open", false),
        );
        association.insert(
            ExtensionCategory::VectorialImage,
            OpenerInfo::new("inkscape", false),
        );
        association.insert(ExtensionCategory::Video, OpenerInfo::new("mpv", false));
        Self { association }
    }

    fn opener_info(&self, ext: &str) -> Option<&OpenerInfo> {
        self.association.get(&ExtensionCategory::parse(ext))
    }

    fn update_from_file(&mut self, yaml: &serde_yaml::value::Value) {
        if let Some(audio) = OpenerInfo::from_yaml(&yaml["audio"]) {
            self.association
                .entry(ExtensionCategory::Audio)
                .and_modify(|e| *e = audio);
        }
        if let Some(bitmap_image) = OpenerInfo::from_yaml(&yaml["bitmap_image"]) {
            self.association
                .entry(ExtensionCategory::BitmapImage)
                .and_modify(|e| *e = bitmap_image);
        }
        if let Some(libreoffice) = OpenerInfo::from_yaml(&yaml["libreoffice"]) {
            self.association
                .entry(ExtensionCategory::LibreOffice)
                .and_modify(|e| *e = libreoffice);
        }
        if let Some(readable) = OpenerInfo::from_yaml(&yaml["readable"]) {
            self.association
                .entry(ExtensionCategory::Readable)
                .and_modify(|e| *e = readable);
        }
        if let Some(text) = OpenerInfo::from_yaml(&yaml["text"]) {
            self.association
                .entry(ExtensionCategory::Text)
                .and_modify(|e| *e = text);
        }
        if let Some(unknown) = OpenerInfo::from_yaml(&yaml["unknown"]) {
            self.association
                .entry(ExtensionCategory::Unknown)
                .and_modify(|e| *e = unknown);
        }
        if let Some(vectorial_image) = OpenerInfo::from_yaml(&yaml["vectorial_image"]) {
            self.association
                .entry(ExtensionCategory::VectorialImage)
                .and_modify(|e| *e = vectorial_image);
        }
        if let Some(video) = OpenerInfo::from_yaml(&yaml["video"]) {
            self.association
                .entry(ExtensionCategory::Video)
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
