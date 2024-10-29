use std::fs::File;

use serde::Serialize;
use serde_yml::{from_reader, to_writer, Error as YamlError, Value as YamlValue};

use crate::common::{tilde, SESSION_PATH};
use crate::io::MIN_WIDTH_FOR_DUAL_PANE;
use crate::log_info;

/// Everything about the current session.
/// We keep track of display settings (metadata, dual pane, second pane as preview).
/// Display hidden files is read from args or set to false by default.
/// Since it's specific to a tab, it's not stored here.
///
/// Reads its display values from a session file and updates them when modified.
/// The file is stored at [`crate::common::SESSION_PATH`] which points to `~/.config/fm/session.yaml`.
/// Unreachable or unreadable files are ignored.
///
/// Holds settings about display :
/// - do we display one or two tabs ? Default to true.
/// - do we display files metadata ? Default to true.
/// - do we use to second pane to preview files ? Default to false.
#[derive(Debug, Serialize)]
pub struct Session {
    /// do we display one or two tabs ?
    dual: bool,
    /// do we display all info or only the filenames ?
    metadata: bool,
    /// use the second pane to preview
    preview: bool,
    /// session filepath
    #[serde(skip_serializing)]
    filepath: String,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            dual: true,
            metadata: true,
            preview: false,
            filepath: tilde(SESSION_PATH).to_string(),
        }
    }
}

impl Session {
    /// Creates a new instance of `DisplaySettings`.
    /// Tries to read them from the session file.
    /// Use default value if the file can't be read.
    pub fn new(width: u16) -> Self {
        Self::default().update_from_config(width)
    }

    fn update_from_config(mut self, width: u16) -> Self {
        let Ok(file) = File::open(&self.filepath) else {
            log_info!("Couldn't open file {file}", file = self.filepath);
            return self;
        };
        let Ok(yaml): Result<YamlValue, YamlError> = from_reader(file) else {
            log_info!(
                "Couldn't parse session from file {file}",
                file = self.filepath
            );
            return self;
        };
        match yaml["dual"] {
            YamlValue::Bool(value) => self.dual = Self::parse_dual_pane(value, width),
            _ => self.dual = true,
        }
        match yaml["metadata"] {
            YamlValue::Bool(value) => self.metadata = value,
            _ => self.metadata = true,
        }
        match yaml["preview"] {
            YamlValue::Bool(value) => self.preview = value,
            _ => self.preview = false,
        }
        self
    }

    fn parse_dual_pane(session_bool: bool, width: u16) -> bool {
        if !Self::display_wide_enough(width) {
            return false;
        }
        session_bool
    }

    pub fn dual(&self) -> bool {
        self.dual
    }

    pub fn metadata(&self) -> bool {
        self.metadata
    }

    pub fn preview(&self) -> bool {
        self.preview
    }

    /// True iff the terminal is wide enough to display two panes
    pub fn display_wide_enough(width: u16) -> bool {
        width >= MIN_WIDTH_FOR_DUAL_PANE
    }

    /// True if we display 2 tabs.
    /// It requires two conditions:
    /// 1. The display should be wide enough, bigger than [`crate::io::MIN_WIDTH_FOR_DUAL_PANE`].
    /// 2. The `dual_tab` setting must be true.
    pub fn use_dual_tab(&self, width: u16) -> bool {
        self.dual && Self::display_wide_enough(width)
    }

    pub fn set_dual(&mut self, dual: bool) {
        self.dual = dual;
        self.update_yaml_file();
    }

    pub fn toggle_dual(&mut self) {
        self.dual = !self.dual;
        self.update_yaml_file();
    }

    pub fn toggle_metadata(&mut self) {
        self.metadata = !self.metadata;
        self.update_yaml_file();
    }

    pub fn toggle_preview(&mut self) {
        self.preview = !self.preview;
        self.update_yaml_file();
    }

    /// Writes itself to the session file.
    /// Does nothing if an error is encountered while creating or writing to the session file.
    fn update_yaml_file(&self) {
        let mut file = match File::create(&self.filepath) {
            Ok(file) => file,
            Err(error) => {
                log_info!(
                    "Couldn't create session {file}. Error: {error:?}",
                    file = self.filepath
                );
                return;
            }
        };
        match to_writer(&mut file, &self) {
            Ok(()) => (),
            Err(e) => log_info!(
                "Couldn't write config to session {file}. Error: {e:?}",
                file = self.filepath
            ),
        }
    }
}
