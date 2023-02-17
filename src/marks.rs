use std::collections::BTreeSet;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use log::info;

use crate::constant_strings_paths::MARKS_FILEPATH;
use crate::fm_error::{FmError, FmResult};
use crate::impl_selectable_content;
use crate::utils::read_lines;

/// Holds the marks created by the user.
/// It's an ordered map between any char (except :) and a PathBuf.
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
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// The number of saved marks
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Reads the marks stored in the config file (~/.config/fm/marks.cfg).
    /// If an invalid marks is read, only the valid ones are kept
    /// and the file is saved again.
    pub fn read_from_config_file() -> Self {
        let path = PathBuf::from(shellexpand::tilde(&MARKS_FILEPATH).to_string());
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
        let marks = Self {
            save_path,
            content,
            index: 0,
            used_chars,
        };
        if must_save {
            info!("Wrong marks found, will save it again");
            let _ = marks.save_marks();
        }
        marks
    }

    /// Returns an optional marks associated to a char bind.
    pub fn get(&self, key: char) -> Option<PathBuf> {
        for (ch, dest) in self.content.iter() {
            if &key == ch {
                return Some(dest.clone());
            }
        }
        None
    }

    fn parse_line(line: Result<String, io::Error>) -> FmResult<(char, PathBuf)> {
        let line = line?;
        let sp: Vec<&str> = line.split(':').collect();
        if sp.len() <= 1 {
            return Err(FmError::custom(
                "marks: parse_line",
                &format!("Invalid mark line: {line}"),
            ));
        }
        if let Some(ch) = sp[0].chars().next() {
            let path = PathBuf::from(sp[1]);
            Ok((ch, path))
        } else {
            Err(FmError::custom(
                "marks: parse line",
                &format!("Invalid first character in: {line}"),
            ))
        }
    }

    /// Store a new mark in the config file.
    /// If an update is done, the marks are saved again.
    pub fn new_mark(&mut self, ch: char, path: PathBuf) -> FmResult<()> {
        if ch == ':' {
            return Err(FmError::custom("new_mark", "':' can't be used as a mark"));
        }
        if self.used_chars.contains(&ch) {
            let mut found_index = None;
            for (index, (k, _)) in self.content.iter().enumerate() {
                if *k == ch {
                    found_index = Some(index);
                    break;
                }
            }
            let Some(found_index) = found_index else {return Ok(())};
            self.content[found_index] = (ch, path);
        } else {
            self.content.push((ch, path))
        }

        self.save_marks()
    }

    fn save_marks(&self) -> FmResult<()> {
        let file = std::fs::File::create(&self.save_path)?;
        let mut buf = BufWriter::new(file);
        for (ch, path) in self.content.iter() {
            writeln!(buf, "{}:{}", ch, Self::path_as_string(path)?)?;
        }
        Ok(())
    }

    fn path_as_string(path: &Path) -> FmResult<String> {
        Ok(path
            .to_str()
            .ok_or_else(|| FmError::custom("path_as_string", "Unreadable path"))?
            .to_owned())
    }

    /// Returns a vector of strings like "d: /dev" for every mark.
    pub fn as_strings(&self) -> Vec<String> {
        self.content
            .iter()
            .map(|(ch, path)| Self::format_mark(ch, path))
            .collect()
    }

    fn format_mark(ch: &char, path: &Path) -> String {
        format!("{}    {}", ch, path.to_string_lossy())
    }
}

type Pair = (char, PathBuf);
impl_selectable_content!(Pair, Marks);
