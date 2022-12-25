use std::fs::{create_dir, remove_dir_all, rename, File, OpenOptions};
use std::io::{prelude::*, BufRead, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::info;
use tuikit::term::Term;

use crate::constant_strings_paths::{TRASH_FILE, TRASH_FILE_TEMP, TRASH_FOLDER};
use crate::copy_move::{copy_move, CopyMove};
use crate::fm_error::{FmError, FmResult};
use crate::impl_selectable_content;
use crate::utils::read_lines;

#[derive(Clone)]
pub struct Trash {
    pub content: Vec<(PathBuf, PathBuf)>,
    term: Arc<Term>,
    index: usize,
    trash_folder: String,
    trash_file: String,
    trash_file_temp: String,
}

impl Trash {
    pub fn parse_trash_file(term: Arc<Term>) -> FmResult<Self> {
        let trash_path = shellexpand::tilde(TRASH_FOLDER).to_string();
        let trash_file = shellexpand::tilde(TRASH_FILE).to_string();
        let trash_file_temp = shellexpand::tilde(TRASH_FILE_TEMP).to_string();
        let mut content = vec![];
        if let Ok(lines) = read_lines(&trash_file) {
            for line_result in lines {
                if let Ok(line) = line_result.as_ref() {
                    let sp: Vec<&str> = line.split(':').collect();
                    if sp.is_empty() || sp.len() < 2 {
                        continue;
                    }
                    let origin = PathBuf::from(sp[0]);
                    let dest = PathBuf::from(sp[1]);
                    content.push((origin, dest));
                }
            }
        }
        Ok(Self {
            content,
            term,
            index: 0,
            trash_folder: trash_path,
            trash_file,
            trash_file_temp,
        })
    }

    pub fn trash(&mut self, origin: PathBuf) -> FmResult<PathBuf> {
        let mut dest = PathBuf::from(self.trash_folder.clone());
        if let Some(file_name) = origin.file_name() {
            if !self.contains(&origin) {
                dest.push(file_name);
                copy_move(
                    CopyMove::Move,
                    vec![origin.clone()],
                    self.trash_folder.to_owned(),
                    self.term.clone(),
                )?;

                self.content.push((origin.clone(), dest.clone()));
                self.write_to_trash_file(origin.clone(), dest.clone())?;
            }
            info!("moved to trash {:?} -> {:?}", origin, dest);
            Ok(dest)
        } else {
            Err(FmError::custom(
                "trash",
                &format!("couldn't trash {:?} - wrong filename", origin),
            ))
        }
    }

    fn contains(&self, origin: &Path) -> bool {
        for (o, _) in self.content.iter() {
            if o == origin {
                return true;
            }
        }
        false
    }

    fn find_dest(&self, origin: &Path) -> Option<PathBuf> {
        for (o, d) in self.content.iter() {
            if o == origin {
                return Some(d.to_owned());
            }
        }
        None
    }

    fn write_to_trash_file(&self, origin: PathBuf, dest: PathBuf) -> FmResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(self.trash_file.clone())?;
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

    pub fn restore(&mut self, origin: PathBuf) -> FmResult<()> {
        let origin_str = path_to_string(&origin)?.to_owned();
        if let Some(dest) = self.find_dest(&origin) {
            let parent = find_parent_as_string(&origin)?;
            if let Some(index) = self.found_in_trash_file(&origin_str) {
                copy_move(
                    CopyMove::Move,
                    vec![dest.clone()],
                    parent,
                    self.term.clone(),
                )?;
                info!(
                    "trash: restoring {:?} <- {:?} - index {}",
                    origin, dest, index
                );
                self.remove_line_from_trash_file(index)?;
                self.content.remove(index);
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
        if let Ok(lines) = read_lines(self.trash_file.clone()) {
            for (index, line_result) in lines.enumerate() {
                if let Ok(line) = line_result.as_ref() {
                    let sp: Vec<&str> = line.split(':').collect();
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
            let file: File = File::open(self.trash_file.clone())?;
            let out_file: File = File::create(self.trash_file_temp.clone())?;

            let reader = BufReader::new(&file);
            let mut writer = BufWriter::new(&out_file);

            for (i, line) in reader.lines().enumerate() {
                let line = line.as_ref()?;
                if i != index {
                    writeln!(writer, "{}", line)?;
                }
            }
        }
        rename(self.trash_file_temp.clone(), self.trash_file.clone())?;
        Ok(())
    }

    pub fn empty_trash(&mut self) -> FmResult<()> {
        remove_dir_all(self.trash_folder.clone())?;
        create_dir(self.trash_folder.clone())?;
        let number_of_elements = self.content.len();

        self.content = vec![];

        File::create(self.trash_file.clone())?;
        info!("Emptied the trash: {} elements removed", number_of_elements);

        Ok(())
    }
}

pub type PathPair = (PathBuf, PathBuf);

impl_selectable_content!(PathPair, Trash);

fn find_parent_as_string(path: &Path) -> FmResult<String> {
    Ok(path
        .parent()
        .ok_or_else(|| FmError::custom("restore", &format!("Couldn't find parent of {:?}", path)))?
        .to_str()
        .ok_or_else(|| FmError::custom("restore", "couldn't parse parent into string"))?
        .to_owned())
}

fn path_to_string(path: &Path) -> FmResult<&str> {
    path.to_str()
        .ok_or_else(|| FmError::custom("restore", "couldn't parse origin into string"))
}
