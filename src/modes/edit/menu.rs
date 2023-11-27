use anyhow::Result;

use crate::modes::{CliApplications, Compresser};

pub struct Menu {
    /// Last sudo command ran
    pub sudo_command: Option<String>,
    /// Compression methods
    pub compression: Compresser,
    /// CLI applications
    pub cli_applications: CliApplications,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            sudo_command: None,
            compression: Compresser::default(),
            cli_applications: CliApplications::default(),
        }
    }
}

impl Menu {}
