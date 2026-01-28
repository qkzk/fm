use anyhow::Result;
use serde_yaml_ng::Mapping;

use crate::app::Status;
use crate::common::{is_in_path, TUIS_PATH};
use crate::io::External;
use crate::modes::{Execute, TerminalApplications};
use crate::{impl_content, impl_draw_menu_with_char, impl_selectable, log_info};

/// Directly open a a TUI application
/// The TUI application shares the same window as fm.
/// If the user picked "shell", we use the environment variable `$SHELL` or `bash` if it's not set.
pub fn open_tui_program<P>(program: &str, current_path: P) -> Result<()>
where
    P: AsRef<std::path::Path>,
{
    if program == "shell" {
        External::open_shell_in_window(current_path)
    } else if is_in_path(program) {
        log_info!("Tui menu execute {program}");
        External::open_command_in_window(&[program], current_path)
    } else {
        log_info!("Tui menu program {program} isn't in path");
        Ok(())
    }
}

impl Execute<()> for String {
    fn execute(&self, status: &Status) -> Result<()> {
        open_tui_program(self, status.current_tab().directory_of_selected()?)
    }
}

/// Tui applications which requires a new terminal for interaction.
#[derive(Clone)]
pub struct TuiApplications {
    pub content: Vec<String>,
    index: usize,
}

impl TuiApplications {
    pub fn setup(&mut self) {
        self.update_from_config(TUIS_PATH);
    }

    pub fn is_not_set(&self) -> bool {
        self.content.len() == 1
    }
}

impl Default for TuiApplications {
    fn default() -> Self {
        let index = 0;
        let content = vec!["shell".to_owned()];
        Self { content, index }
    }
}

impl TerminalApplications<String, ()> for TuiApplications {
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

impl_content!(TuiApplications, String);
impl_draw_menu_with_char!(TuiApplications, String);
