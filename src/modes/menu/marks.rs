use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::common::{read_lines, tilde, MARKS_FILEPATH};
use crate::io::DrawMenu;
use crate::{impl_content, impl_selectable, log_info, log_line};

/// Holds the marks created by the user.
/// It's an ordered map between any char (except :) and a `PathBuf`.
#[derive(Clone, Default)]
pub struct Marks {
    save_path: PathBuf,
    content: Vec<(char, PathBuf)>,
    pub index: usize,
    used_chars: BTreeSet<char>,
    paths_to_mark: BTreeMap<PathBuf, char>,
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
    pub fn setup(&mut self) {
        self.save_path = PathBuf::from(tilde(MARKS_FILEPATH).as_ref());
        self.content = vec![];
        self.used_chars = BTreeSet::new();
        let mut must_save = false;
        if let Ok(lines) = read_lines(&self.save_path) {
            for line in lines {
                if let Ok((ch, path)) = Self::parse_line(line) {
                    if !self.used_chars.contains(&ch) {
                        self.content.push((ch, path));
                        self.used_chars.insert(ch);
                    }
                } else {
                    must_save = true;
                }
            }
        }
        self.content.sort();
        self.index = 0;
        if must_save {
            log_info!("Wrong marks found, will save it again");
            let _ = self.save_marks();
        }
        self.save_paths_to_mark();
    }

    fn save_paths_to_mark(&mut self) {
        self.paths_to_mark.clear();
        for (c, p) in self.content.iter() {
            self.paths_to_mark.insert(p.to_path_buf(), *c);
        }
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
                bail!(
                    "marks: parse line
Invalid first character in: {line}"
                )
            },
            |ch| {
                if ch == ':' || ch == ' ' || ch.is_control() {
                    bail!(
                        "marks: parse line
Invalid first characer in: {line}"
                    )
                }
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
        if ch.is_control() {
            log_line!("new mark - please use a printable symbol for mark");
            return Ok(());
        }
        if ch == ':' || ch == ' ' {
            log_line!("new mark - '{ch}' can't be used as a mark");
            return Ok(());
        }
        if self.used_chars.contains(&ch) {
            self.update_mark(ch, path);
        } else {
            self.content.push((ch, path.to_path_buf()));
            self.used_chars.insert(ch);
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
        if self.index >= self.content.len() {
            bail!(
                "index of mark is {index} and len is {len}",
                index = self.index,
                len = self.content.len()
            );
        }

        let (ch, path) = &self.content[self.index];
        self.used_chars.remove(ch);
        log_line!("Removed marks {ch} -> {path}", path = path.display());
        self.content.remove(self.index);
        self.prev();
        self.save_marks()?;
        Ok(())
    }

    fn save_marks(&mut self) -> Result<()> {
        let file = std::fs::File::create(&self.save_path)?;
        let mut buf = BufWriter::new(file);
        self.content.sort();
        self.save_paths_to_mark();
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

    /// Returns the char for associated to a path.
    /// For marked path, it's their mark,
    /// Otherwise it's ' '
    pub fn char_for(&self, path: &Path) -> &char {
        self.paths_to_mark.get(path).unwrap_or(&' ')
    }

    pub fn move_path(&mut self, old_path: &Path, new_path: &Path) -> Result<()> {
        let ch = *self.char_for(old_path);
        if ch == ' ' {
            return Ok(());
        }
        self.update_mark(ch, new_path);
        self.save_marks()
    }
}

type Pair = (char, PathBuf);
impl_content!(Marks, Pair);

impl DrawMenu<Pair> for Marks {}
