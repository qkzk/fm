use std::fs::File;

use anyhow::Result;
use serde::Serialize;
use serde_yaml::{Error as YamlError, Value as YamlValue};

use crate::common::SESSION_PATH;
use crate::io::MIN_WIDTH_FOR_DUAL_PANE;
use crate::log_info;

/// Reads its display values from a session file and updates them when modified.
/// The file is stored at [`crate::common::SESSION_PATH`] which points to `~/.config/fm/session.yaml`.
/// Unreachable or unreadable files are ignored.
///
/// Holds settings about display :
/// - do we display one or two tabs ? Default to true.
/// - do we display files metadata ? Default to true.
/// - do we use to second pane to preview files ? Default to false.
#[derive(Debug, Serialize)]
pub struct DisplaySettings {
    /// do we display one or two tabs ?
    pub dual: bool,
    /// do we display all info or only the filenames ?
    pub metadata: bool,
    /// use the second pane to preview
    pub preview: bool,
    /// session filepath
    #[serde(skip_serializing)]
    filepath: String,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            dual: true,
            metadata: true,
            preview: false,
            filepath: shellexpand::tilde(SESSION_PATH).to_string(),
        }
    }
}

impl DisplaySettings {
    pub fn new(width: usize) -> Self {
        Self::default().update_from_config(width)
    }

    fn update_from_config(mut self, width: usize) -> Self {
        let Ok(file) = File::open(&self.filepath) else {
            log_info!("Couldn't open file {file}", file = self.filepath);
            return self;
        };
        let Ok(yaml): Result<YamlValue, YamlError> = serde_yaml::from_reader(file) else {
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
        log_info!("{self:?}");
        self
    }

    fn parse_dual_pane(session_bool: bool, width: usize) -> bool {
        if !Self::display_wide_enough(width) {
            return false;
        }
        session_bool
    }

    /// True iff the terminal is wide enough to display two panes
    ///
    /// # Errors
    ///
    /// Fail if the terminal has crashed
    pub fn display_wide_enough(width: usize) -> bool {
        width >= MIN_WIDTH_FOR_DUAL_PANE
    }

    pub fn use_dual_tab(&self, width: usize) -> bool {
        self.dual && Self::display_wide_enough(width)
    }

    pub fn set_dual(&mut self, dual: bool) {
        self.dual = dual;
        match self.update_yaml_file() {
            Ok(()) => (),
            Err(error) => log_info!("Error while updating session file {error:?}"),
        };
    }

    pub fn toggle_dual(&mut self) {
        self.dual = !self.dual;
        match self.update_yaml_file() {
            Ok(()) => (),
            Err(error) => log_info!("Error while updating session file {error:?}"),
        };
    }

    pub fn toggle_metadata(&mut self) {
        self.metadata = !self.metadata;
        match self.update_yaml_file() {
            Ok(()) => (),
            Err(error) => log_info!("Error while updating session file {error:?}"),
        };
    }

    pub fn toggle_preview(&mut self) {
        self.preview = !self.preview;
        match self.update_yaml_file() {
            Ok(()) => (),
            Err(error) => log_info!("Error while updating session file {error:?}"),
        };
    }

    fn update_yaml_file(&self) -> Result<()> {
        let mut file = File::create(&self.filepath)?;
        serde_yaml::to_writer(&mut file, &self)?;

        Ok(())
    }
}
