use std::collections::BTreeMap;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use log::info;

use crate::constant_strings_paths::MARKS_FILEPATH;
use crate::fm_error::{FmError, FmResult};
use crate::utils::read_lines;

/// Holds the marks created by the user.
/// It's an ordered map between any char (except :) and a PathBuf.
pub struct Marks {
    save_path: PathBuf,
    marks: BTreeMap<char, PathBuf>,
}

impl Marks {
    /// True if there's no marks yet
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// The number of saved marks
    pub fn len(&self) -> usize {
        self.marks.len()
    }

    /// Reads the marks stored in the config file (~/.config/fm/marks.cfg).
    /// If an invalid marks is read, only the valid ones are kept
    /// and the file is saved again.
    pub fn read_from_config_file() -> Self {
        let path = PathBuf::from(shellexpand::tilde(&MARKS_FILEPATH).to_string());
        Self::read_from_file(path)
    }

    fn read_from_file(save_path: PathBuf) -> Self {
        let mut marks = BTreeMap::new();
        let mut must_save = false;
        if let Ok(lines) = read_lines(&save_path) {
            for line in lines {
                if let Ok((ch, path)) = Self::parse_line(line) {
                    marks.insert(ch, path);
                } else {
                    must_save = true;
                }
            }
        }
        let marks = Self { save_path, marks };
        if must_save {
            info!("Wrong marks found, will save it again");
            let _ = marks.save_marks();
        }
        marks
    }

    /// Returns an optional marks associated to a char bind.
    pub fn get(&self, ch: char) -> Option<&PathBuf> {
        self.marks.get(&ch)
    }

    fn parse_line(line: Result<String, io::Error>) -> FmResult<(char, PathBuf)> {
        let line = line?;
        let sp: Vec<&str> = line.split(':').collect();
        if sp.len() <= 1 {
            return Err(FmError::custom(
                "marks: parse_line",
                &format!("Invalid mark line: {}", line),
            ));
        }
        if let Some(ch) = sp[0].chars().next() {
            let path = PathBuf::from(sp[1]);
            Ok((ch, path))
        } else {
            Err(FmError::custom(
                "marks: parse line",
                &format!("Invalid first character in: {}", line),
            ))
        }
    }

    /// Store a new mark in the config file.
    /// All the marks are saved again.
    pub fn new_mark(&mut self, ch: char, path: PathBuf) -> FmResult<()> {
        if ch == ':' {
            return Err(FmError::custom("new_mark", "':' can't be used as a mark"));
        }
        self.marks.insert(ch, path);
        self.save_marks()
    }

    fn save_marks(&self) -> FmResult<()> {
        let file = std::fs::File::create(&self.save_path)?;
        let mut buf = BufWriter::new(file);
        for (ch, path) in self.marks.iter() {
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
        self.marks
            .iter()
            .map(|(ch, path)| Self::format_mark(ch, path))
            .collect()
    }

    fn format_mark(ch: &char, path: &Path) -> String {
        format!("{}    {}", ch, path.to_string_lossy())
    }
}
