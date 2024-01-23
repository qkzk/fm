use std::fmt::Display;
use strum_macros::Display;

use anyhow::{anyhow, Context, Result};

use crate::{common::read_lines, impl_content, impl_selectable};

pub struct InputHistory {
    file_path: std::path::PathBuf,
    content: Vec<HistoryElement>,
    index: usize,
}

impl InputHistory {
    pub fn load(path: &str) -> Result<Self> {
        Ok(Self {
            file_path: std::path::PathBuf::from(path),
            content: Self::load_content(path)?,
            index: 0,
        })
    }

    fn load_content(path: &str) -> Result<Vec<HistoryElement>> {
        Ok(read_lines(path)?
            .map(|line| HistoryElement::from_str(line))
            .filter_map(|line| line.ok())
            .collect())
    }

    pub fn write(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Display)]
pub enum HistoryKind {
    Cd,
    Search,
    Exec,
    Action,
    Rename,
    Chmod,
    Newfile,
    Newdir,
    RegexMatch,
    Sort,
    Filter,
    SetNvimAddr,
    Shell,
    Remote,
}

impl HistoryKind {
    fn from_str(kind: &str) -> Result<Self> {
        Ok(match kind {
            "Cd" => Self::Cd,
            "Search" => Self::Search,
            "Exec" => Self::Exec,
            "Action" => Self::Action,
            "Rename" => Self::Rename,
            "Chmod" => Self::Chmod,
            "Newfile" => Self::Newfile,
            "Newdir" => Self::Newdir,
            "RegexMatch" => Self::RegexMatch,
            "Sort" => Self::Sort,
            "Filter" => Self::Filter,
            "SetNvimAddr" => Self::SetNvimAddr,
            "Shell" => Self::Shell,
            "Remote" => Self::Remote,
            _ => return Err(anyhow!("{kind} isn't a valid HistoryKind")),
        })
    }
}

pub struct HistoryElement {
    kind: HistoryKind,
    content: String,
}

impl Display for HistoryElement {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{kind} - {content}",
            kind = self.kind,
            content = self.content
        )
    }
}

impl HistoryElement {
    fn split_kind_content<'a>(line: Result<String, std::io::Error>) -> Result<(&'a str, &'a str)> {
        let line = line?;
        let (mut kind, mut content) = line
            .split_once('-')
            .context("no delimiter '-' found in line")?;
        kind = kind.trim();
        content = content.trim();
        Ok((kind, content))
    }

    fn from_str(line: Result<String, std::io::Error>) -> Result<Self> {
        let (kind, content) = Self::split_kind_content(line)?;
        if content.is_empty() {
            Err(anyhow!("empty line"))
        } else {
            Ok(Self {
                kind: HistoryKind::from_str(kind)?,
                content: content.to_owned(),
            })
        }
    }
}

impl_content!(HistoryElement, InputHistory);
impl_selectable!(InputHistory);
