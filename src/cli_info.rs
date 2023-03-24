use std::collections::HashMap;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use log::info;

use crate::impl_selectable_content;

use crate::utils::is_program_in_path;

/// Holds the command line commands we can run and display
/// without leaving FM.
/// Those are non interactive commands displaying some info about the current
/// file tree or setup.
#[derive(Clone)]
pub struct CliInfo {
    pub content: Vec<&'static str>,
    commands: HashMap<&'static str, Vec<&'static str>>,
    index: usize,
}

impl Default for CliInfo {
    fn default() -> Self {
        let index = 0;
        let commands = HashMap::from([
            ("duf", vec!["duf"]),
            ("inxi", vec!["inxi", "-FB", "--color"]),
            ("neofetch", vec!["neofetch"]),
            ("lsusb", vec!["lsusb"]),
        ]);
        let content: Vec<&'static str> = commands
            .keys()
            .filter(|s| is_program_in_path(s))
            .copied()
            .collect();

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
        let key = self.selected().context("no cli selected")?;
        let output = {
            let args = self.commands.get(key).context("no arguments for exe")?;
            info!("execute. executable: {key}, arguments: {args:?}",);
            let child = Command::new(args[0])
                .args(&args[1..])
                .env("CLICOLOR_FORCE", "1")
                .env("COLORTERM", "ansi")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;
            let output = child.wait_with_output()?;
            if output.status.success() {
                Ok(String::from_utf8(output.stdout)?)
            } else {
                Err(anyhow!("execute: command didn't finished correctly",))
            }
        }?;
        Ok(output)
    }
}

type StaticStr = &'static str;
impl_selectable_content!(StaticStr, CliInfo);
