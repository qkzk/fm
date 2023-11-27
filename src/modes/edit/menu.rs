use anyhow::Result;

#[derive(Clone, Debug)]
pub struct Menu {
    /// Last sudo command ran
    pub sudo_command: Option<String>,
}

impl Default for Menu {
    fn default() -> Self {
        Self { sudo_command: None }
    }
}

impl Menu {}
