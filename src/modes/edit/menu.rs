use anyhow::Result;

use super::Compresser;

#[derive(Debug)]
pub struct Menu {
    /// Last sudo command ran
    pub sudo_command: Option<String>,
    /// Compression methods
    pub compression: Compresser,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            compression: Compresser::default(),
            sudo_command: None,
        }
    }
}

impl Menu {}
