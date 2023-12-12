use anyhow::Result;

use crate::app::Status;
use crate::common::is_program_in_path;
use crate::common::CLI_INFO;
use crate::impl_selectable_content;
use crate::io::execute_with_ansi_colors;
use crate::log_info;
use crate::log_line;
use crate::modes::ShellCommandParser;

/// A command line application launcher.
/// It's constructed from a line in [`crate::common::CLI_INFO`].
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
    pub executable: &'static str,
    /// The full command with parsable arguments like %s
    parsable_command: &'static str,
    /// A single line description of the command
    pub desc: &'static str,
}

impl CliCommand {
    fn new(desc_command: (&'static str, &'static str)) -> Option<Self> {
        let desc = desc_command.0;
        let parsable_command = desc_command.1;
        let args = parsable_command.split(' ').collect::<Vec<_>>();
        let Some(executable) = args.first() else {
            return None;
        };
        if is_program_in_path(*executable) {
            Some(Self {
                executable,
                parsable_command,
                desc,
            })
        } else {
            None
        }
    }

    /// Run its parsable command and capture its output.
    /// Some environement variables are first set to ensure the colored output.
    /// Long running commands may freeze the display.
    fn execute(&self, status: &Status) -> Result<(String, String)> {
        let args = ShellCommandParser::new(self.parsable_command).compute(status)?;
        log_info!("execute. {args:?}");
        log_line!("Executed {args:?}");

        let params: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let command_output = execute_with_ansi_colors(&params)?;
        let text_output = String::from_utf8(command_output.stdout)?;
        if command_output.status.success() {
            log_info!(
                "Command {a} exited with error code {e}",
                a = args[0],
                e = command_output.status
            );
        };
        Ok((text_output, self.parsable_command.to_owned()))
    }
}

/// Holds the command line commands we can run and display
/// without leaving FM.
/// Those are non interactive commands displaying some info about the current
/// file tree or setup.
#[derive(Clone)]
pub struct CliApplications {
    pub content: Vec<CliCommand>,
    index: usize,
}

impl Default for CliApplications {
    fn default() -> Self {
        let index = 0;
        let content = CLI_INFO
            .iter()
            .map(|line| CliCommand::new(*line))
            .map(|opt_command| opt_command.unwrap())
            .collect();
        Self { content, index }
    }
}

impl CliApplications {
    /// Run the selected command and capture its output.
    /// Some environement variables are first set to ensure the colored output.
    /// Long running commands may freeze the display.
    pub fn execute(&self, status: &Status) -> Result<(String, String)> {
        self.content[self.index].execute(status)
    }
}

impl_selectable_content!(CliCommand, CliApplications);
