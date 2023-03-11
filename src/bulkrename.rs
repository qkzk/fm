use anyhow::{anyhow, Result};
use log::info;
use rand::Rng;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::constant_strings_paths::TMP_FOLDER_PATH;
use crate::impl_selectable_content;
use crate::opener::Opener;
use crate::status::Status;

/// Struct holding informations about files about to be renamed.
/// We only need to know which are the original filenames and which
/// temporary file is used to modify them.
/// This feature is a poor clone of ranger's one.
pub struct Bulkrename<'a> {
    original_filepath: Option<Vec<&'a Path>>,
    parent_dir: Option<&'a str>,
    temp_file: PathBuf,
}

impl<'a> Bulkrename<'a> {
    /// Creates a new Bulkrename instance.
    pub fn renamer(original_filepath: Vec<&'a Path>) -> Result<Self> {
        let temp_file = Self::generate_random_filepath()?;
        Ok(Self {
            original_filepath: Some(original_filepath),
            parent_dir: None,
            temp_file,
        })
    }

    pub fn creator(path_str: &'a str) -> Result<Self> {
        let temp_file = Self::generate_random_filepath()?;
        info!("created {temp_file:?}");
        Ok(Self {
            original_filepath: None,
            parent_dir: Some(path_str),
            temp_file,
        })
    }

    /// Rename the files.
    /// The tempory file is opened with our `Opener` crate, allowing us
    /// to use the default text file editor.
    /// Filenames are sanitized before processing.
    fn rename(&mut self, opener: &Opener) -> Result<()> {
        self.write_original_names()?;
        let original_modification = Self::get_modified_date(&self.temp_file)?;
        self.open_temp_file_with_editor(opener)?;

        Self::watch_modification_in_thread(&self.temp_file, original_modification)?;

        self.rename_all(self.get_new_filenames()?)?;
        self.delete_temp_file()
    }

    fn create_files(&mut self, opener: &Opener) -> Result<()> {
        self.create_random_file()?;
        let original_modification = Self::get_modified_date(&self.temp_file)?;
        self.open_temp_file_with_editor(opener)?;

        Self::watch_modification_in_thread(&self.temp_file, original_modification)?;

        self.create_all_files(&self.get_new_filenames()?)?;
        self.delete_temp_file()
    }

    fn watch_modification_in_thread(
        filepath: &Path,
        original_modification: SystemTime,
    ) -> Result<()> {
        let filepath = filepath.to_owned();
        let handle = thread::spawn(move || loop {
            if Self::is_file_modified(&filepath, original_modification).unwrap_or(true) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        });
        match handle.join() {
            Ok(handle) => Ok(handle),
            Err(e) => Err(anyhow!("watching thread failed {e:?}")),
        }
    }

    fn get_modified_date(filepath: &Path) -> Result<SystemTime> {
        Ok(std::fs::metadata(filepath)?.modified()?)
    }

    fn random_name() -> String {
        let mut rand_str = String::with_capacity(14);
        rand_str.push_str("fm-");
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(7)
            .for_each(|ch| rand_str.push(ch as char));
        rand_str.push_str(".txt");
        rand_str
    }

    fn generate_random_filepath() -> Result<PathBuf> {
        let mut filepath = PathBuf::from(&TMP_FOLDER_PATH);
        filepath.push(Self::random_name());
        Ok(filepath)
    }

    fn create_random_file(&self) -> Result<()> {
        std::fs::File::create(&self.temp_file)?;
        Ok(())
    }

    fn write_original_names(&self) -> Result<()> {
        let mut file = std::fs::File::create(&self.temp_file)?;

        for path in self.original_filepath.clone().unwrap().iter() {
            let Some(os_filename) = path.file_name() else { return Ok(()) };
            let Some(filename) = os_filename.to_str() else {return Ok(()) };
            let b = filename.as_bytes();
            file.write_all(b)?;
            file.write_all(&[b'\n'])?;
        }
        Ok(())
    }

    fn open_temp_file_with_editor(&self, opener: &Opener) -> Result<()> {
        info!("opening tempory file {:?}", self.temp_file);
        opener.open(&self.temp_file)
    }

    fn is_file_modified(path: &Path, original_modification: std::time::SystemTime) -> Result<bool> {
        let last_modification = Self::get_modified_date(path)?;
        Ok(last_modification > original_modification)
    }

    fn get_new_filenames(&self) -> Result<Vec<String>> {
        let file = std::fs::File::open(&self.temp_file)?;
        let reader = std::io::BufReader::new(file);

        let new_names: Vec<String> = reader
            .lines()
            .flatten()
            .map(|line| line.trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect();
        if let Some(original_filepath) = self.original_filepath.clone() {
            if new_names.len() < original_filepath.len() {
                return Err(anyhow!("new filenames: not enough filenames"));
            }
        }
        Ok(new_names)
    }

    fn delete_temp_file(&self) -> Result<()> {
        std::fs::remove_file(&self.temp_file)?;
        Ok(())
    }

    fn rename_all(&self, new_filenames: Vec<String>) -> Result<()> {
        let mut counter = 0;
        for (path, filename) in self
            .original_filepath
            .clone()
            .unwrap()
            .iter()
            .zip(new_filenames.iter())
        {
            let new_name = sanitize_filename::sanitize(filename);
            self.rename_file(path, &new_name)?;
            counter += 1;
            info!(target: "special", "Bulk renamed {path} to {new_name}", path=path.display())
        }
        info!(target: "special", "Bulk renamed {counter} files");
        Ok(())
    }

    fn create_all_files(&self, new_filenames: &[String]) -> Result<()> {
        let mut counter = 0;
        for filename in new_filenames.iter() {
            let mut new_path = std::path::PathBuf::from(self.parent_dir.unwrap());
            if !filename.ends_with('/') {
                new_path.push(filename);
                let Some(parent) = new_path.parent() else { return Ok(()); };
                info!("Bulk new files. Creating parent: {}", parent.display());
                if std::fs::create_dir_all(parent).is_err() {
                    continue;
                };
                info!("creating: {new_path:?}");
                std::fs::File::create(&new_path)?;
                info!(target:"special", "Bulk created {new_path}", new_path=new_path.display());
                counter += 1;
            } else {
                new_path.push(filename);
                info!("Bulk creating dir: {}", new_path.display());
                std::fs::create_dir_all(&new_path)?;
                info!(target:"special", "Bulk created {new_path}", new_path=new_path.display());
                counter += 1;
            }
        }
        info!(target: "special", "Bulk created {counter} files");
        Ok(())
    }

    fn rename_file(&self, path: &Path, filename: &str) -> Result<()> {
        let mut parent = PathBuf::from(path);
        parent.pop();
        std::fs::rename(path, parent.join(filename))?;
        Ok(())
    }
}

pub struct Bulk {
    pub content: Vec<String>,
    index: usize,
}

impl Default for Bulk {
    fn default() -> Self {
        Self {
            content: vec![
                "Rename files".to_owned(),
                "New files or folders. End folder with a slash '/'".to_owned(),
            ],
            index: 0,
        }
    }
}

impl Bulk {
    /// Execute the selected bulk method depending on the index.
    /// First method is a rename of selected files,
    /// Second is the creation of files,
    /// Third is the creation of folders.
    pub fn execute_bulk(&self, status: &Status) -> Result<()> {
        match self.index {
            0 => Bulkrename::renamer(status.filtered_flagged_files())?.rename(&status.opener),
            1 => Bulkrename::creator(status.selected_path_str())?.create_files(&status.opener),
            _ => Ok(()),
        }
    }
}

impl_selectable_content!(String, Bulk);
