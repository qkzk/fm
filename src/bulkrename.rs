use log::info;
use rand::Rng;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::constant_strings_paths::TMP_FOLDER_PATH;
use crate::fm_error::{FmError, FmResult};
use crate::opener::Opener;

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
    pub fn renamer(original_filepath: Vec<&'a Path>) -> FmResult<Self> {
        let temp_file = Self::generate_random_filepath()?;
        Ok(Self {
            original_filepath: Some(original_filepath),
            parent_dir: None,
            temp_file,
        })
    }

    pub fn creator(path_str: &'a str) -> FmResult<Self> {
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
    pub fn rename(&mut self, opener: &Opener) -> FmResult<()> {
        self.write_original_names()?;
        let original_modification = Self::get_modified_date(&self.temp_file)?;
        self.open_temp_file_with_editor(opener)?;

        Self::watch_modification_in_thread(&self.temp_file, original_modification)?;

        self.rename_all(self.get_new_filenames()?)?;
        self.delete_temp_file()
    }

    pub fn create(&mut self, opener: &Opener) -> FmResult<()> {
        self.create_random_file()?;
        let original_modification = Self::get_modified_date(&self.temp_file)?;
        self.open_temp_file_with_editor(opener)?;

        Self::watch_modification_in_thread(&self.temp_file, original_modification)?;

        self.create_all(self.get_new_filenames()?)?;
        self.delete_temp_file()
    }

    fn watch_modification_in_thread(
        filepath: &Path,
        original_modification: SystemTime,
    ) -> FmResult<()> {
        let filepath = filepath.to_owned();
        let handle = thread::spawn(move || loop {
            if Self::is_file_modified(&filepath, original_modification).unwrap_or(true) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        });
        Ok(handle.join()?)
    }

    fn get_modified_date(filepath: &Path) -> FmResult<SystemTime> {
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

    fn generate_random_filepath() -> FmResult<PathBuf> {
        let mut filepath = PathBuf::from(&TMP_FOLDER_PATH);
        filepath.push(Self::random_name());
        Ok(filepath)
    }

    fn create_random_file(&self) -> FmResult<()> {
        std::fs::File::create(&self.temp_file)?;
        Ok(())
    }

    fn write_original_names(&self) -> FmResult<()> {
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

    fn open_temp_file_with_editor(&self, opener: &Opener) -> FmResult<()> {
        info!("opengin {:?}", self.temp_file);
        opener.open(&self.temp_file)
    }

    fn is_file_modified(
        path: &Path,
        original_modification: std::time::SystemTime,
    ) -> FmResult<bool> {
        let last_modification = Self::get_modified_date(path)?;
        Ok(last_modification > original_modification)
    }

    fn get_new_filenames(&self) -> FmResult<Vec<String>> {
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
                return Err(FmError::custom("new filenames", "not enough filenames"));
            }
        }
        Ok(new_names)
    }

    fn delete_temp_file(&self) -> FmResult<()> {
        std::fs::remove_file(&self.temp_file)?;
        Ok(())
    }

    fn rename_all(&self, new_filenames: Vec<String>) -> FmResult<()> {
        for (path, filename) in self
            .original_filepath
            .clone()
            .unwrap()
            .iter()
            .zip(new_filenames.iter())
        {
            self.rename_file(path, &sanitize_filename::sanitize(filename))?
        }
        Ok(())
    }

    fn create_all(&self, new_filenames: Vec<String>) -> FmResult<()> {
        for filename in new_filenames.iter() {
            let filename = sanitize_filename::sanitize(filename);
            let mut new_path = std::path::PathBuf::from(self.parent_dir.unwrap());
            new_path.push(filename);
            info!("creating: {new_path:?}");
            std::fs::File::create(new_path)?;
        }
        Ok(())
    }

    fn rename_file(&self, path: &Path, filename: &str) -> FmResult<()> {
        let mut parent = PathBuf::from(path);
        parent.pop();
        std::fs::rename(path, parent.join(filename))?;
        Ok(())
    }
}
