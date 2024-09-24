use std::collections::BTreeSet;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::common::{read_lines, tilde};
use crate::{impl_content, impl_selectable, log_info, log_line};

/// Holds the marks created by the user.
/// It's an ordered map between any char (except :) and a `PathBuf`.
#[derive(Clone)]
pub struct Marks {
    save_path: PathBuf,
    content: Vec<(char, PathBuf)>,
    /// The currently selected shortcut
    pub index: usize,
    used_chars: BTreeSet<char>,
}

impl Marks {
    /// True if there's no marks yet
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// The number of saved marks
    #[must_use]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Reads the marks stored in the config file (~/.config/fm/marks.cfg).
    /// If an invalid marks is read, only the valid ones are kept
    /// and the file is saved again.
    #[must_use]
    pub fn new(config_path: &str) -> Self {
        let path = PathBuf::from(tilde(config_path).to_string());
        Self::read_from_file(path)
    }

    fn read_from_file(save_path: PathBuf) -> Self {
        let mut content = vec![];
        let mut must_save = false;
        let mut used_chars = BTreeSet::new();
        if let Ok(lines) = read_lines(&save_path) {
            for line in lines {
                if let Ok((ch, path)) = Self::parse_line(line) {
                    if !used_chars.contains(&ch) {
                        content.push((ch, path));
                        used_chars.insert(ch);
                    }
                } else {
                    must_save = true;
                }
            }
        }
        content.sort();
        let mut marks = Self {
            save_path,
            content,
            index: 0,
            used_chars,
        };
        if must_save {
            log_info!("Wrong marks found, will save it again");
            let _ = marks.save_marks();
        }
        marks
    }

    /// Returns an optional marks associated to a char bind.
    #[must_use]
    pub fn get(&self, key: char) -> Option<PathBuf> {
        for (ch, dest) in &self.content {
            if &key == ch {
                return Some(dest.clone());
            }
        }
        None
    }

    fn parse_line(line: Result<String, io::Error>) -> Result<(char, PathBuf)> {
        let line = line?;
        let sp: Vec<&str> = line.split(':').collect();
        if sp.len() != 2 {
            return Err(anyhow!("marks: parse_line: Invalid mark line: {line}"));
        }
        sp[0].chars().next().map_or_else(
            || {
                Err(anyhow!(
                    "marks: parse line
                 Invalid first character in: {line}"
                ))
            },
            |ch| {
                let path = PathBuf::from(sp[1]);
                Ok((ch, path))
            },
        )
    }

    /// Store a new mark in the config file.
    /// If an update is done, the marks are saved again.
    ///
    /// # Errors
    ///
    /// It may fail if writing to the marks file fails.
    pub fn new_mark(&mut self, ch: char, path: &Path) -> Result<()> {
        if ch == ':' {
            log_line!("new mark - ':' can't be used as a mark");
            return Ok(());
        }
        if self.used_chars.contains(&ch) {
            self.update_mark(ch, path);
        } else {
            self.content.push((ch, path.to_path_buf()));
        }

        self.save_marks()?;
        log_line!("Saved mark {ch} -> {p}", p = path.display());
        Ok(())
    }

    fn update_mark(&mut self, ch: char, path: &Path) {
        let mut found_index = None;
        for (index, (k, _)) in self.content.iter().enumerate() {
            if *k == ch {
                found_index = Some(index);
                break;
            }
        }
        if let Some(found_index) = found_index {
            self.content[found_index] = (ch, path.to_path_buf());
        }
    }

    pub fn remove_selected(&mut self) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }
        let (ch, path) = self.selected().context("no marks saved")?;
        log_line!("Removed marks {ch} -> {path}", path = path.display());
        self.content.remove(self.index);
        self.prev();
        self.save_marks()
    }

    fn save_marks(&mut self) -> Result<()> {
        let file = std::fs::File::create(&self.save_path)?;
        let mut buf = BufWriter::new(file);
        self.content.sort();
        for (ch, path) in &self.content {
            writeln!(buf, "{}:{}", ch, Self::path_as_string(path)?)?;
        }
        Ok(())
    }

    fn path_as_string(path: &Path) -> Result<String> {
        Ok(path
            .to_str()
            .context("path_as_string: unreadable path")?
            .to_owned())
    }

    /// Returns a vector of strings like "d: /dev" for every mark.
    #[must_use]
    pub fn as_strings(&self) -> Vec<String> {
        self.content
            .iter()
            .map(|(ch, path)| Self::format_mark(*ch, path))
            .collect()
    }

    fn format_mark(ch: char, path: &Path) -> String {
        format!("{ch}    {path}", path = path.display())
    }
}

type Pair = (char, PathBuf);
impl_selectable!(Marks);
impl_content!(Pair, Marks);
