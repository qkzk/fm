use std::fmt::Display;
use std::io::Write;

use clap::Parser;
use strum_macros::Display;

use anyhow::{anyhow, bail, Context, Result};

use crate::common::{read_lines, tilde};
use crate::io::Args;
use crate::modes::{InputCompleted, InputSimple, Menu};

/// The whole input history, read and written from and to a file.
/// It's filtered by content.
/// If the flag "log_are_enabled" is set to false, it will not be updated in the logs.
pub struct InputHistory {
    file_path: std::path::PathBuf,
    content: Vec<HistoryElement>,
    filtered: Vec<HistoryElement>,
    index: usize,
    log_are_enabled: bool,
}

impl InputHistory {
    pub fn load(path: &str) -> Result<Self> {
        let file_path = std::path::PathBuf::from(tilde(path).to_string());
        Ok(Self {
            content: Self::load_content(&file_path)?,
            file_path,
            filtered: vec![],
            index: 0,
            log_are_enabled: Args::parse().log,
        })
    }

    fn load_content(path: &std::path::Path) -> Result<Vec<HistoryElement>> {
        if !std::path::Path::new(&path).exists() {
            std::fs::File::create(path)?;
        }
        Ok(read_lines(path)?
            .map(HistoryElement::from_str)
            .filter_map(|line| line.ok())
            .collect())
    }

    pub fn write_elem(&self, elem: &HistoryElement) -> Result<()> {
        let mut hist_file = std::fs::OpenOptions::new()
            .append(true)
            .open(&self.file_path)?;
        hist_file.write_all(elem.to_string().as_bytes())?;
        Ok(())
    }

    pub fn filter_by_mode(&mut self, edit_mode: Menu) {
        let Some(kind) = HistoryKind::from_mode(edit_mode) else {
            return;
        };
        self.index = 0;
        self.filtered = self
            .content
            .iter()
            .filter(|elem| elem.kind == kind)
            .rev()
            .map(|elem| elem.to_owned())
            .collect()
    }

    pub fn next(&mut self) {
        if !self.filtered.is_empty() {
            self.index = (self.index + 1) % self.filtered.len();
        }
    }

    pub fn prev(&mut self) {
        if self.index > 0 {
            self.index -= 1
        } else if !self.filtered.is_empty() {
            self.index = self.filtered.len() - 1
        }
    }

    pub fn current(&self) -> Option<&str> {
        let elem = self.filtered.get(self.index)?;
        Some(&elem.content)
    }

    /// If logs are disabled, nothing is saved on disk, only during current session
    pub fn update(&mut self, mode: Menu, input_string: &str) -> Result<()> {
        let Some(elem) = HistoryElement::from_mode_input_string(mode, input_string) else {
            return Ok(());
        };
        if self.log_are_enabled {
            self.write_elem(&elem)?;
        }
        self.content.push(elem);
        Ok(())
    }
}

/// Different kind of histories, depending of the edit_mode.
/// It has a few methods to record and filter methods from text input.
#[derive(Display, Clone, PartialEq, Eq)]
pub enum HistoryKind {
    InputSimple(InputSimple),
    InputCompleted(InputCompleted),
}

impl HistoryKind {
    fn from_string(kind: &String) -> Result<Self> {
        Ok(match kind.as_ref() {
            "Cd" => Self::InputCompleted(InputCompleted::Cd),
            "Search" => Self::InputCompleted(InputCompleted::Search),
            "Exec" => Self::InputCompleted(InputCompleted::Exec),
            "Action" => Self::InputCompleted(InputCompleted::Action),

            "Shell" => Self::InputSimple(InputSimple::ShellCommand),
            "Chmod" => Self::InputSimple(InputSimple::Chmod),
            "Sort" => Self::InputSimple(InputSimple::Sort),
            "Rename" => Self::InputSimple(InputSimple::Rename),
            "Newfile" => Self::InputSimple(InputSimple::Newfile),
            "Newdir" => Self::InputSimple(InputSimple::Newdir),
            "RegexMatch" => Self::InputSimple(InputSimple::RegexMatch),
            "Filter" => Self::InputSimple(InputSimple::Filter),
            "SetNvimAddr" => Self::InputSimple(InputSimple::SetNvimAddr),
            "Remote" => Self::InputSimple(InputSimple::Remote),

            _ => bail!("{kind} isn't a valid HistoryKind"),
        })
    }

    fn from_mode(edit_mode: Menu) -> Option<Self> {
        match edit_mode {
            Menu::InputSimple(InputSimple::Password(_, _) | InputSimple::CloudNewdir) => None,
            Menu::InputSimple(input_simple) => Some(Self::InputSimple(input_simple)),
            Menu::InputCompleted(input_completed) => Some(Self::InputCompleted(input_completed)),
            _ => None,
        }
    }
}

/// Simple struct to record what kind of history is related to an input.
/// Since we record most user inputs, they are messed up.
/// Navigating in those elements can be confusing if we don't filter them by kind.
#[derive(Clone)]
pub struct HistoryElement {
    kind: HistoryKind,
    content: String,
}

impl Display for HistoryElement {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(
            f,
            "{kind} - {content}",
            kind = self.kind,
            content = self.content
        )
    }
}

impl HistoryElement {
    fn split_kind_content(line: Result<String, std::io::Error>) -> Result<(String, String)> {
        let line = line?.to_owned();
        let (mut kind, mut content) = line
            .split_once('-')
            .context("no delimiter '-' found in line")?;
        kind = kind.trim();
        content = content.trim();
        Ok((kind.to_owned(), content.to_owned()))
    }

    pub fn from_mode_input_string(mode: Menu, input_string: &str) -> Option<Self> {
        let kind = HistoryKind::from_mode(mode)?;
        Some(Self {
            kind,
            content: input_string.to_owned(),
        })
    }

    fn from_str(line: Result<String, std::io::Error>) -> Result<Self> {
        let (kind, content) = Self::split_kind_content(line)?;
        if content.is_empty() {
            Err(anyhow!("empty line"))
        } else {
            Ok(Self {
                kind: HistoryKind::from_string(&kind)?,
                content: content.to_owned(),
            })
        }
    }

    pub fn for_display(&self) -> &str {
        &self.content
    }
}
