use std::fmt::{Display, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{Error as IoError, Write};
use std::path::{Path, PathBuf};

use clap::Parser;

use anyhow::{bail, Context, Result};

use crate::common::{read_lines, tilde};
use crate::io::Args;
use crate::modes::{InputCompleted, InputSimple, Menu};

/// The whole input history, read and written from and to a file.
/// It's filtered by content.
/// If the flag "log_are_enabled" is set to false, it will not be updated in the logs.
pub struct InputHistory {
    file_path: PathBuf,
    content: Vec<HistoryElement>,
    filtered: Vec<HistoryElement>,
    index: Option<usize>,
    log_are_enabled: bool,
}

impl InputHistory {
    pub fn load(path: &str) -> Result<Self> {
        let file_path = PathBuf::from(tilde(path).to_string());
        Ok(Self {
            content: Self::load_content(&file_path)?,
            file_path,
            filtered: vec![],
            index: None,
            log_are_enabled: Args::parse().run_args.log,
        })
    }

    fn load_content(path: &Path) -> Result<Vec<HistoryElement>> {
        if !Path::new(&path).exists() {
            File::create(path)?;
        }
        Ok(read_lines(path)?
            .map(HistoryElement::from_str)
            .filter_map(|line| line.ok())
            .collect())
    }

    fn write_elem(&self, elem: &HistoryElement) -> Result<()> {
        let mut hist_file = OpenOptions::new().append(true).open(&self.file_path)?;
        hist_file.write_all(elem.to_string().as_bytes())?;
        Ok(())
    }

    pub fn filter_by_mode(&mut self, menu_mode: Menu) {
        let Some(kind) = HistoryKind::from_mode(menu_mode) else {
            return;
        };
        self.index = None;
        self.filtered = self
            .content
            .iter()
            .filter(|elem| elem.kind == kind)
            .map(|elem| elem.to_owned())
            .collect()
    }

    pub fn prev(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.index.is_none() {
            self.index = Some(0);
        } else {
            self.index = self.index.map(|index| (index + 1) % self.filtered.len());
        }
    }

    pub fn next(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.index.is_none() {
            self.index = Some(self.filtered.len().saturating_sub(1));
        } else {
            self.index = self.index.map(|index| {
                if index > 0 {
                    index - 1
                } else {
                    self.filtered.len() - 1
                }
            })
        }
    }

    pub fn current(&self) -> Option<&HistoryElement> {
        match self.index {
            None => None,
            Some(index) => self.filtered.get(index),
        }
    }

    /// If logs are disabled, nothing is saved on disk, only during current session
    pub fn update(&mut self, mode: Menu, input_string: &str) -> Result<()> {
        let Some(elem) = HistoryElement::from_mode_input_string(mode, input_string) else {
            return Ok(());
        };
        if let Some(last) = self.filtered.last() {
            if *last == elem {
                return Ok(());
            }
        }
        if self.log_are_enabled {
            self.write_elem(&elem)?;
        }
        self.content.push(elem);
        Ok(())
    }

    /// True iff the mode is logged.
    /// It's almost always the case, only password mode isn't saved.
    /// This method is usefull to check if an input should be replaced when the user want to.
    pub fn is_mode_logged(&self, mode: &Menu) -> bool {
        !matches!(
            mode,
            Menu::Navigate(_)
                | Menu::InputSimple(InputSimple::Password(_, _))
                | Menu::InputSimple(InputSimple::CloudNewdir)
                | Menu::NeedConfirmation(_)
        )
    }
}

/// Different kind of histories, depending of the menu_mode.
/// It has a few methods to record and filter methods from text input.
#[derive(Clone, PartialEq, Eq)]
pub enum HistoryKind {
    InputSimple(InputSimple),
    InputCompleted(InputCompleted),
}

impl Display for HistoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let menu = match self {
            Self::InputCompleted(input_completed) => match input_completed {
                InputCompleted::Cd => "Cd",
                InputCompleted::Search => "Search",
                InputCompleted::Exec => "Exec",
                InputCompleted::Action => "Action",
            },
            Self::InputSimple(input_simple) => match input_simple {
                InputSimple::Rename => "Rename",
                InputSimple::Chmod => "Chmod",
                InputSimple::Newfile => "Newfile",
                InputSimple::Newdir => "Newdir",
                InputSimple::RegexMatch => "RegexMatch",
                InputSimple::Sort => "Sort",
                InputSimple::Filter => "Filter",
                InputSimple::SetNvimAddr => "SetNvimAddr",
                InputSimple::ShellCommand => "ShellCommand",
                InputSimple::Remote => "Remote",
                InputSimple::CloudNewdir => "xxx",
                InputSimple::Password(_, _) => "xxx",
            },
        };
        write!(f, "{menu}")
    }
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

    fn from_mode(menu_mode: Menu) -> Option<Self> {
        match menu_mode {
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
#[derive(Clone, Eq, PartialEq)]
pub struct HistoryElement {
    kind: HistoryKind,
    content: String,
}

impl Display for HistoryElement {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        writeln!(
            f,
            "{kind} - {content}",
            kind = self.kind,
            content = self.content
        )
    }
}

impl HistoryElement {
    fn split_kind_content(line: Result<String, IoError>) -> Result<(String, String)> {
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

    fn from_str(line: Result<String, IoError>) -> Result<Self> {
        let (kind, content) = Self::split_kind_content(line)?;
        if content.is_empty() {
            bail!("empty line")
        } else {
            Ok(Self {
                kind: HistoryKind::from_string(&kind)?,
                content: content.to_owned(),
            })
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}
