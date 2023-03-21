use anyhow::{Context, Result};

use crate::impl_selectable_content;

use crate::opener::execute_and_capture_output;
use crate::status::Status;
use crate::utils::is_program_in_path;

#[derive(Clone)]
pub struct CliInfo {
    pub content: Vec<String>,
    index: usize,
}

impl Default for CliInfo {
    fn default() -> Self {
        let index = 0;
        let content = vec!["duf".to_owned(), "inxi".to_owned()];
        Self { content, index }
    }
}

impl CliInfo {
    pub fn execute(&self, status: &Status) -> Result<String> {
        let exe = self.selected().context("no cli selected")?;
        let output = execute_and_capture_output(exe, &vec![])?;
        Ok(output)
    }
}

impl_selectable_content!(String, CliInfo);
