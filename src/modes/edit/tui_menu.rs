use anyhow::{Context, Result};

use crate::app::Status;
use crate::common::is_program_in_path;
use crate::impl_selectable_content;
use crate::io::{execute_in_child_without_output, execute_in_child_without_output_with_path};
use crate::log_line;

#[derive(Clone)]
pub struct TuiApplications {
    pub content: Vec<(String, bool)>,
    index: usize,
}

impl Default for TuiApplications {
    fn default() -> Self {
        let index = 0;
        let content = vec![("shell".to_owned(), false)];
        Self { content, index }
    }
}

impl TuiApplications {
    /// Creates a new shell menu instance, parsing the `config_file`.
    ///
    /// # Errors
    ///
    /// It may fail if the file content can't be parsed or if the file can't be read.
    pub fn new(config_file: &str) -> Result<Self> {
        let mut shell_menu = Self::default();
        let file = std::fs::File::open(std::path::Path::new(
            &shellexpand::tilde(config_file).to_string(),
        ))?;
        let yaml = serde_yaml::from_reader(file)?;
        shell_menu.update_from_file(&yaml);
        Ok(shell_menu)
    }

    fn update_from_file(&mut self, yaml: &serde_yaml::mapping::Mapping) {
        for (key, mapping) in yaml {
            let Some(command) = key.as_str() else {
                continue;
            };
            if !is_program_in_path(command) {
                continue;
            }
            let command = command.to_owned();
            let Some(require_cwd) = mapping.get("cwd") else {
                continue;
            };
            let Some(require_cwd) = require_cwd.as_bool() else {
                continue;
            };
            self.content.push((command, require_cwd));
        }
    }

    /// Execute the selected command
    ///
    /// # Errors
    ///
    /// May fail if the current directory has no parent aka /
    /// May fail if the command itself fails.
    pub fn execute(&self, status: &Status) -> Result<()> {
        let (name, require_cwd) = &self.content[self.index];
        if name.as_str() == "shell" {
            Self::require_cwd(status)?;
        } else if *require_cwd {
            Self::require_cwd_and_command(status, name.as_str())?;
        } else {
            Self::simple(status, name.as_str())?;
        };
        log_line!("Executed {name}");
        Ok(())
    }

    /// Execute a command requiring to be ran from current working directory.
    ///
    /// # Errors
    ///
    /// May fail if the current directory has no parent aka /
    /// May fail if the command itself fails.
    pub fn require_cwd_and_command(status: &Status, command: &str) -> Result<()> {
        let tab = status.selected_non_mut();
        let path = tab
            .directory_of_selected()?
            .to_str()
            .context("event_shell: couldn't parse the directory")?;
        execute_in_child_without_output(&status.opener.terminal, &["-d", path, "-e", command])?;
        Ok(())
    }

    fn simple(status: &Status, command: &str) -> Result<()> {
        execute_in_child_without_output(&status.opener.terminal, &["-e", command])?;
        Ok(())
    }

    fn _simple_with_args(status: &Status, args: &[&str]) -> Result<()> {
        execute_in_child_without_output(&status.opener.terminal, args)?;
        Ok(())
    }

    fn require_cwd(status: &Status) -> Result<()> {
        let tab = status.selected_non_mut();
        let path = tab.directory_of_selected()?;
        execute_in_child_without_output_with_path(&status.opener.terminal, path, None)?;
        Ok(())
    }
}

type SBool = (String, bool);

impl_selectable_content!(SBool, TuiApplications);
