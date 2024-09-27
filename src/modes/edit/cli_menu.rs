use anyhow::Context;
use anyhow::Result;
use serde_yml::from_reader;
use serde_yml::Mapping;

use crate::app::Status;
use crate::common::{is_in_path, tilde};
use crate::io::{execute_with_ansi_colors, DrawMenu, ToPrint};
use crate::modes::{Navigate, ShellCommandParser};
use crate::{impl_content, impl_selectable, log_info, log_line};

pub trait Execute<T> {
    fn execute(&self, status: &Status) -> Result<T>;
}

/// A command line application launcher.
/// It's constructed from a line in a config file.
/// Each command has a short description, a name (first word of second element)
/// and a list of parsable parameters.
/// See [`crate::modes::ShellCommandParser`] for a description of accetable tokens.
///
/// Only commands which are in `$PATH` at runtime are built from `Self::new(...)`,
/// Commands which aren't accessible return `None`
///
/// Those commands should output a string (therefore be command line).
/// No interaction with the user is possible.
#[derive(Clone)]
pub struct CliCommand {
    /// The executable itself like `ls`
    pub executable: String,
    /// The full command with parsable arguments like %s
    parsable_command: String,
    /// A single line description of the command
    pub desc: String,
}

impl CliCommand {
    fn new(desc: String, args: String) -> Option<Self> {
        let executable = args.split(' ').next()?;
        if !is_in_path(executable) {
            return None;
        }
        let desc = desc.replace('_', " ");
        Some(Self {
            executable: executable.to_owned(),
            parsable_command: args,
            desc,
        })
    }
}

impl Execute<(String, String)> for CliCommand {
    /// Run its parsable command and capture its output.
    /// Some environement variables are first set to ensure the colored output.
    /// Long running commands may freeze the display.
    fn execute(&self, status: &Status) -> Result<(String, String)> {
        let args = ShellCommandParser::new(&self.parsable_command).compute(status)?;
        log_info!("execute. {args:?}");
        log_line!("Executed {args:?}");

        let params: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let command_output = execute_with_ansi_colors(&params)?;
        let text_output = String::from_utf8(command_output.stdout)?;
        if !command_output.status.success() {
            log_info!(
                "Command {a} exited with error code {e}",
                a = args[0],
                e = command_output.status
            );
        };
        Ok((text_output, self.parsable_command.to_owned()))
    }
}

pub trait CLApplications<T: Execute<U>, U>: Sized + Default + Content<T> {
    fn new(config_file: &str) -> Self {
        Self::default().update_from_config(config_file)
    }

    fn update_from_config(mut self, config_file: &str) -> Self {
        let Ok(file) = std::fs::File::open(std::path::Path::new(&tilde(config_file).to_string()))
        else {
            log_info!("Couldn't open cli file at {config_file}. Using default");
            return self;
        };
        match from_reader(file) {
            Ok(yaml) => {
                self.parse_yaml(&yaml);
            }
            Err(error) => {
                log_info!("error parsing yaml file {config_file}. Error: {error:?}");
            }
        }
        self
    }

    fn parse_yaml(&mut self, yaml: &Mapping);

    /// Run the selected command and capture its output.
    /// Some environement variables are first set to ensure the colored output.
    /// Long running commands may freeze the display.
    fn execute(&self, status: &Status) -> Result<U> {
        self.selected().context("")?.execute(status)
    }
}

/// Holds the command line commands we can run and display
/// without leaving FM.
/// Those are non interactive commands displaying some info about the current
/// file tree or setup.
#[derive(Clone, Default)]
pub struct CliApplications {
    pub content: Vec<CliCommand>,
    index: usize,
    pub desc_size: usize,
}

impl CliApplications {
    pub fn update_desc_size(mut self) -> Self {
        let desc_size = self
            .content
            .iter()
            .map(|cli| cli.desc.len())
            .fold(usize::MIN, |a, b| a.max(b));
        self.desc_size = desc_size;
        self
    }
}

impl CLApplications<CliCommand, (String, String)> for CliApplications {
    fn parse_yaml(&mut self, yaml: &Mapping) {
        for (key, mapping) in yaml {
            let Some(name) = key.as_str() else {
                continue;
            };
            let Some(command) = mapping.get("command") else {
                continue;
            };
            let Some(command) = command.as_str() else {
                continue;
            };
            let Some(cli_command) = CliCommand::new(name.to_owned(), command.to_owned()) else {
                continue;
            };
            self.content.push(cli_command)
        }
    }
}

impl_selectable!(CliApplications);
impl_content!(CliCommand, CliApplications);

impl ToPrint for CliCommand {
    fn to_print(&self) -> String {
        let desc_size = 20_usize.saturating_sub(self.desc.len());
        format!(
            "{desc}{space:<desc_size$}{exe}",
            desc = self.desc,
            exe = self.executable,
            space = " "
        )
    }
}

impl DrawMenu<Navigate, CliCommand> for CliApplications {}
