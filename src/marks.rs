use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufWriter, Error, ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::fm_error::FmError;
use crate::fm_error::FmResult;

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
        if let Ok(lines) = read_lines(&save_path) {
            for line in lines {
                if let Ok((ch, path)) = Self::parse_line(line) {
                    marks.insert(ch, path);
                }
            }
        }
        Self { save_path, marks }
    }

    pub fn get(&self, ch: char) -> Option<&PathBuf> {
        self.marks.get(&ch)
    }

    fn parse_line(line: Result<String, io::Error>) -> Result<(char, PathBuf), io::Error> {
        let line = line?;
        let sp: Vec<&str> = line.split(':').collect();
        if let Some(ch) = sp[0].chars().next() {
            let path = PathBuf::from(sp[1]);
            Ok((ch, path))
        } else {
            Err(Error::new(ErrorKind::InvalidData, "Invalid char"))
        }
    }

    pub fn new_mark(&mut self, ch: char, path: PathBuf) -> FmResult<()> {
        if ch == ':' {
            return Err(crate::fm_error::FmError::new("':' can't be used as a mark"));
        }
        self.marks.insert(ch, path);
        self.save_marks()
    }

    fn save_marks(&self) -> FmResult<()> {
        if !self.save_path.exists() {
            let _ = std::fs::File::create(&self.save_path);
            eprintln!("Created a file for marks in {:?}", &self.save_path);
        }

        let file = OpenOptions::new().write(true).open(&self.save_path)?;
        let mut buf = BufWriter::new(file);

        for (ch, path) in self.marks.iter() {
            let _ = writeln!(buf, "{}:{}", ch, Self::path_as_string(path)?);
        }
        Ok(())
    }

    fn path_as_string(path: &Path) -> FmResult<String> {
        Ok(path
            .to_str()
            .ok_or_else(|| FmError::new("Unreadable path"))?
            .to_owned())
    }

    pub fn as_strings(&self) -> FmResult<Vec<String>> {
        Ok(self
            .marks
            .iter()
            .map(|(ch, path)| Self::format_mark(ch, path).unwrap_or_default())
            .collect())
    }

    fn format_mark(ch: &char, path: &Path) -> FmResult<String> {
        let mut s = "".to_owned();
        s.push(*ch);
        s.push_str("   ");
        s.push_str(
            path.to_str()
                .ok_or_else(|| FmError::new("Unreadable path"))?,
        );
        s.push('\n');
        Ok(s)
    }
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}