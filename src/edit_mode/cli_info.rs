use std::process::{Command, Stdio};

use anyhow::Result;
use log::info;

use crate::constant_strings_paths::CLI_INFO_COMMANDS;
use crate::impl_selectable_content;
use crate::log_line;
use crate::utils::is_program_in_path;

/// Holds the command line commands we can run and display
/// without leaving FM.
/// Those are non interactive commands displaying some info about the current
/// file tree or setup.
#[derive(Clone)]
pub struct CliInfo {
    pub content: Vec<&'static str>,
    commands: Vec<Vec<&'static str>>,
    index: usize,
}

impl Default for CliInfo {
    fn default() -> Self {
        let index = 0;
        let commands: Vec<Vec<&str>> = CLI_INFO_COMMANDS
            .iter()
            .map(|command| command.split(' ').collect::<Vec<_>>())
            .filter(|args| is_program_in_path(args[0]))
            .collect();

        let content: Vec<&str> = commands.iter().map(|args| args[0]).collect();

        Self {
            content,
            index,
            commands,
        }
    }
}

impl CliInfo {
    /// Run the selected command and capture its output.
    /// Some environement variables are first set to ensure the colored output.
    /// Long running commands may freeze the display.
    pub fn execute(&self) -> Result<String> {
        let args = self.commands[self.index].clone();
        info!("execute. {args:?}");
        log_line!("Executed {args:?}");
        let child = Command::new(args[0])
            .args(&args[1..])
            .env("CLICOLOR_FORCE", "1")
            .env("COLORTERM", "ansi")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let command_output = child.wait_with_output()?;
        let text_output = {
            if command_output.status.success() {
                String::from_utf8(command_output.stdout)?
            } else {
                format!(
                    "Command {a} exited with error code {e}",
                    a = args[0],
                    e = command_output.status
                )
            }
        };
        Ok(text_output)
    }
}

type StaticStr = &'static str;
impl_selectable_content!(StaticStr, CliInfo);
