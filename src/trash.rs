use std::collections::BTreeMap;
use std::fs::{rename, File, OpenOptions};
use std::io::{self, prelude::*, BufRead, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::info;
use tuikit::term::Term;

use crate::copy_move::{copy_move, CopyMove};
use crate::fm_error::{FmError, FmResult};

// TODO! make it tilde expand machin
pub static TRASH_PATH: &str = "/home/quentin/.config/fm/trash/";
pub static TRASH_FILE: &str = "/home/quentin/.config/fm/trash_file";
pub static TRASH_FILE_TEMP: &str = "/tmp/trash_file_temp";

// TODO! make it a navigable content
#[derive(Clone)]
pub struct Trash {
    pub content: BTreeMap<PathBuf, PathBuf>,
    term: Arc<Term>,
}

impl Trash {
    pub fn new(term: Arc<Term>) -> Self {
        Self {
            content: BTreeMap::new(),
            term,
        }
    }

    pub fn trash(&mut self, origin: PathBuf) -> FmResult<PathBuf> {
        let mut dest = PathBuf::from(TRASH_PATH);
        if let Some(file_name) = origin.file_name() {
            dest.push(file_name);
            copy_move(
                CopyMove::Move,
                vec![origin.clone()],
                TRASH_PATH.to_owned(),
                self.term.clone(),
            )?;
            self.content.insert(origin.clone(), dest.clone());
            self.write_to_trash_file(origin.clone(), dest.clone())?;
            Ok(dest)
        } else {
            Err(FmError::custom(
                "trash",
                &format!("couldn't trash {:?} - wrong filename", origin),
            ))
        }
    }

    fn write_to_trash_file(&self, origin: PathBuf, dest: PathBuf) -> FmResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(TRASH_FILE)?;
        if let Err(e) = writeln!(
            file,
            "{}:{}",
            origin
                .to_str()
                .ok_or_else(|| FmError::custom("write_to_trash_file", "couldn't read origin"))?,
            dest.to_str()
                .ok_or_else(|| FmError::custom("write_to_trash_file", "couldn't read dest"))?,
        ) {
            info!("Couldn't write to trash file: {}", e)
        }
        Ok(())
    }

    pub fn restore(&mut self, origin_str: String) -> FmResult<()> {
        let origin = PathBuf::from(origin_str.clone());
        if let Some(dest) = self.content.get(&origin) {
            if let Some(index) = self.found_in_trash_file(&origin_str) {
                copy_move(
                    CopyMove::Move,
                    vec![dest.clone()],
                    origin_str,
                    self.term.clone(),
                )?;
                self.remove_line_from_trash_file(index)?;
            } else {
                return Err(FmError::custom(
                    "restore",
                    &format!("Couldn't find {} in trash file", origin_str),
                ));
            }
            Ok(())
        } else {
            Err(FmError::custom(
                "restore",
                &format!("Couldn't restore {}", origin_str),
            ))
        }
    }

    fn found_in_trash_file(&self, origin_str: &str) -> Option<usize> {
        if let Ok(lines) = read_lines(TRASH_FILE) {
            for (index, line_result) in lines.enumerate() {
                if let Ok(line) = line_result.as_ref() {
                    let sp: Vec<&str> = line.split(":").collect();
                    if sp.is_empty() {
                        continue;
                    }
                    let origin_line = sp[0];
                    if origin_line.starts_with(origin_str) {
                        return Some(index);
                    }
                }
            }
        }
        None
    }

    fn remove_line_from_trash_file(&mut self, index: usize) -> FmResult<()> {
        {
            let file: File = File::open(TRASH_FILE)?;
            let out_file: File = File::open(TRASH_FILE_TEMP)?;

            let reader = BufReader::new(&file);
            let mut writer = BufWriter::new(&out_file);

            for (i, line) in reader.lines().enumerate() {
                let line = line.as_ref()?;
                if i != index {
                    writeln!(writer, "{}", line)?;
                }
            }
        }
        rename(TRASH_FILE_TEMP, TRASH_FILE)?;
        Ok(())
    }

    // TODO!
    pub fn empty_trash(&mut self) -> FmResult<()> {
        Ok(())
    }

    // TODO!
    pub fn parse_trash_file(term: Arc<Term>) -> FmResult<Self> {
        Ok(Self::new(term))
    }
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
