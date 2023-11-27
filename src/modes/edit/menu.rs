use anyhow::Result;

use crate::common::TUIS_PATH;
use crate::modes::CliApplications;
use crate::modes::Compresser;
use crate::modes::TuiApplications;

pub struct Menu {
    /// Last sudo command ran
    pub sudo_command: Option<String>,
    /// Compression methods
    pub compression: Compresser,
    /// CLI applications
    pub cli_applications: CliApplications,
    /// TUI application
    pub tui_applications: TuiApplications,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            sudo_command: None,
            compression: Compresser::default(),
            cli_applications: CliApplications::default(),
            tui_applications: TuiApplications::new(TUIS_PATH),
        }
    }
}

impl Menu {}
