use anyhow::Result;
use serde_yml::Mapping;

use crate::app::Status;
use crate::common::is_in_path;
use crate::io::{execute_without_output, execute_without_output_with_path};
use crate::log_line;
use crate::modes::{CLApplications, Execute};
use crate::{impl_content, impl_selectable};

/// Execute a command requiring to be ran from current working directory.
///
/// # Errors
///
/// May fail if the current directory has no parent aka /
/// May fail if the command itself fails.
fn require_cwd_and_command(status: &Status, command: &str) -> Result<()> {
    execute_without_output(
        &status.internal_settings.opener.terminal,
        &[&status.internal_settings.opener.terminal_flag, command],
    )?;
    Ok(())
}

fn execute_shell(status: &Status) -> Result<()> {
    let tab = status.current_tab();
    let path = tab.directory_of_selected()?;
    execute_without_output_with_path(&status.internal_settings.opener.terminal, path, None)?;
    Ok(())
}

/// Directly open a a TUI application
pub fn open_tui_program(status: &mut Status, program: &str) -> Result<()> {
    if is_in_path(program) {
        require_cwd_and_command(status, program)
    } else {
        Ok(())
    }
}

impl Execute<()> for String {
    fn execute(&self, status: &Status) -> Result<()> {
        if self.as_str() == "shell" {
            execute_shell(status)?;
        } else {
            require_cwd_and_command(status, self)?;
        };
        log_line!("Executed {name}", name = self);
        Ok(())
    }
}

#[derive(Clone)]
pub struct TuiApplications {
    pub content: Vec<String>,
    index: usize,
}

impl Default for TuiApplications {
    fn default() -> Self {
        let index = 0;
        let content = vec!["shell".to_owned()];
        Self { content, index }
    }
}

impl CLApplications<String, ()> for TuiApplications {
    fn parse_yaml(&mut self, yaml: &Mapping) {
        for (key, _) in yaml {
            let Some(command) = key.as_str() else {
                continue;
            };
            if is_in_path(command) {
                self.content.push(command.to_owned());
            }
        }
    }
}

impl_selectable!(TuiApplications);
impl_content!(String, TuiApplications);

use crate::io::DrawMenu;
use crate::modes::Navigate;

impl DrawMenu<Navigate, String> for TuiApplications {}
