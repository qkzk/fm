use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::fm_error::{ErrorVariant, FmError, FmResult};

static MARKS_FILEPATH: &str = "~/.config/fm/marks.cfg";

pub struct Marks {
    save_path: PathBuf,
    marks: HashMap<char, PathBuf>,
}

impl Marks {
    pub fn read_from_config_file() -> Self {
        let path = PathBuf::from(shellexpand::tilde(&MARKS_FILEPATH).to_string());
        Self::read_from_file(path)
    }

    fn read_from_file(save_path: PathBuf) -> Self {
        let mut marks = HashMap::new();
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
            eprintln!("Wrong marks found, will save it again");
            let _ = marks.save_marks();
        }
        marks
    }

    pub fn get(&self, ch: char) -> Option<&PathBuf> {
        self.marks.get(&ch)
    }

    fn parse_line(line: Result<String, io::Error>) -> FmResult<(char, PathBuf)> {
        let line = line?;
        let sp: Vec<&str> = line.split(':').collect();
        if sp.len() <= 1 {
            return Err(FmError::new(
                ErrorVariant::CUSTOM("marks: parse_line".to_owned()),
                "Invalid mark line",
            ));
        }
        if let Some(ch) = sp[0].chars().next() {
            let path = PathBuf::from(sp[1]);
            Ok((ch, path))
        } else {
            Err(FmError::new(
                ErrorVariant::CUSTOM("marks: parse line".to_owned()),
                "Invalid char",
            ))
        }
    }

    pub fn new_mark(&mut self, ch: char, path: PathBuf) -> FmResult<()> {
        if ch == ':' {
            return Err(FmError::new(
                ErrorVariant::CUSTOM("new_mark".to_owned()),
                "':' can't be used as a mark",
            ));
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
            .ok_or_else(|| {
                FmError::new(
                    ErrorVariant::CUSTOM("path_as_string".to_owned()),
                    "Unreadable path",
                )
            })?
            .to_owned())
    }

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

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
