use anyhow::{Context, Result};

use crate::app::Status;
use crate::common::is_program_in_path;
use crate::impl_content;
use crate::impl_selectable;
use crate::io::{execute_without_output, execute_without_output_with_path};
use crate::log_info;
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
    pub fn new(config_file: &str) -> Self {
        Self::default().update_from_config(config_file)
    }

    fn update_from_config(mut self, config_file: &str) -> Self {
        let Ok(file) = std::fs::File::open(std::path::Path::new(
            &shellexpand::tilde(config_file).to_string(),
        )) else {
            log_info!("Couldn't open tuis file at {config_file}. Using default");
            return self;
        };
        let Ok(yaml) = serde_yaml::from_reader(file) else {
            log_info!("Couldn't parse tuis file at {config_file}. Using default");
            return self;
        };
        self.parse_yaml(&yaml);
        self
    }

    fn parse_yaml(&mut self, yaml: &serde_yaml::mapping::Mapping) {
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
        let tab = status.current_tab();
        let path = tab
            .directory_of_selected()?
            .to_str()
            .context("event_shell: couldn't parse the directory")?;
        execute_without_output(
            &status.internal_settings.opener.terminal,
            &["-d", path, "-e", command],
        )?;
        Ok(())
    }

    fn simple(status: &Status, command: &str) -> Result<()> {
        execute_without_output(&status.internal_settings.opener.terminal, &["-e", command])?;
        Ok(())
    }

    fn require_cwd(status: &Status) -> Result<()> {
        let tab = status.current_tab();
        let path = tab.directory_of_selected()?;
        execute_without_output_with_path(&status.internal_settings.opener.terminal, path, None)?;
        Ok(())
    }

    /// Directly open a a TUI application
    pub fn open_program(status: &mut Status, program: &str) -> Result<()> {
        if is_program_in_path(program) {
            TuiApplications::require_cwd_and_command(status, program)
        } else {
            Ok(())
        }
    }
}

type SBool = (String, bool);

// impl_selectable_content!(SBool, TuiApplications);
impl_selectable!(TuiApplications);
impl_content!(SBool, TuiApplications);
