use std::fmt::Display;
use std::io::Write;

use clap::Parser;
use strum_macros::Display;

use anyhow::{anyhow, bail, Context, Result};

use crate::common::{read_lines, tilde};
use crate::io::Args;
use crate::modes::{Edit, InputCompleted, InputSimple};

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

    pub fn filter_by_mode(&mut self, edit_mode: Edit) {
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
    pub fn update(&mut self, mode: Edit, input_string: &str) -> Result<()> {
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

            "Shell" => Self::InputSimple(InputSimple::Shell),
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

    fn from_mode(edit_mode: Edit) -> Option<Self> {
        match edit_mode {
            Edit::InputSimple(InputSimple::Password(_, _) | InputSimple::CloudNewdir) => None,
            Edit::InputSimple(input_simple) => Some(Self::InputSimple(input_simple)),
            Edit::InputCompleted(input_completed) => Some(Self::InputCompleted(input_completed)),
            _ => None,
        }
    }
}

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

    pub fn from_mode_input_string(mode: Edit, input_string: &str) -> Option<Self> {
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
