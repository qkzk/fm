use rand::Rng;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::fm_error::{FmError, FmResult};
use crate::opener::{ExtensionKind, Opener};

static TMP_FOLDER: &str = "/tmp";

pub struct Bulkrename<'a> {
    original_filepath: Vec<&'a Path>,
    temp_file: PathBuf,
}

impl<'a> Bulkrename<'a> {
    pub fn new(original_filepath: Vec<&'a Path>) -> Result<Self, FmError> {
        let temp_file = Self::generate_random_filepath()?;
        Ok(Self {
            original_filepath,
            temp_file,
        })
    }

    pub fn rename(&mut self, opener: &Opener) -> Result<(), FmError> {
        self.write_original_names()?;
        let original_modification = Self::get_modified_date(&self.temp_file)?;
        self.open_temp_file_with_editor(opener)?;

        Self::watch_modification_in_thread(self.temp_file.clone(), original_modification)?;

        self.rename_all(self.get_new_filenames()?)?;
        self.delete_temp_file()
    }

    fn watch_modification_in_thread(
        filepath: PathBuf,
        original_modification: SystemTime,
    ) -> FmResult<()> {
        let handle = thread::spawn(move || loop {
            if Self::is_file_modified(&filepath, original_modification).unwrap_or(true) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        });
        Ok(handle.join()?)
    }

    fn get_modified_date(filepath: &PathBuf) -> Result<SystemTime, FmError> {
        Ok(std::fs::metadata(filepath)?.modified()?)
    }

    fn random_name() -> String {
        let mut rand_str = String::with_capacity(10);
        rand_str.push_str("fm-");
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(7)
            .for_each(|ch| rand_str.push(ch as char));
        rand_str
    }

    fn generate_random_filepath() -> Result<PathBuf, FmError> {
        let mut filepath = PathBuf::from(&TMP_FOLDER);
        filepath.push(Self::random_name());
        Ok(filepath)
    }

    fn write_original_names(&self) -> Result<(), FmError> {
        let mut file = std::fs::File::create(&self.temp_file)?;
        for path in self.original_filepath.iter() {
            if let Some(os_filename) = path.file_name() {
                if let Some(filename) = os_filename.to_str() {
                    let b = filename.as_bytes();
                    file.write_all(b)?;
                    file.write_all(&[b'\n'])?;
                }
            }
        }
        Ok(())
    }

    fn open_temp_file_with_editor(&self, opener: &Opener) -> Result<(), FmError> {
        let filepath = &self.temp_file;
        if let Some(editor_info) = opener.get(ExtensionKind::Text) {
            opener.open_with(editor_info, filepath.to_owned())?;
        } else {
            return Err(FmError::new(
                "Markdown files should have a dedicated opener",
            ));
        }
        Ok(())
    }

    fn is_file_modified(
        path: &PathBuf,
        original_modification: std::time::SystemTime,
    ) -> Result<bool, FmError> {
        let last_modification = Self::get_modified_date(path)?;
        Ok(last_modification > original_modification)
    }

    fn get_new_filenames(&self) -> Result<Vec<String>, FmError> {
        let file = std::fs::File::open(&self.temp_file)?;

        let reader = std::io::BufReader::new(file);
        let mut new_names = vec![];
        for line in reader.lines() {
            let line2 = line?;
            let line = line2.trim();
            if line.is_empty() {
                return Err(FmError::new("empty filename"));
            }
            new_names.push(line2);
        }
        if new_names.len() < self.original_filepath.len() {
            return Err(FmError::new("not enough filenames"));
        }
        Ok(new_names)
    }

    fn delete_temp_file(&self) -> Result<(), FmError> {
        let filepath = &self.temp_file;
        std::fs::remove_file(filepath)?;
        Ok(())
    }

    fn rename_all(&self, new_filenames: Vec<String>) -> Result<(), FmError> {
        for (path, filename) in self.original_filepath.iter().zip(new_filenames.iter()) {
            self.rename_file(path, filename)?
        }
        Ok(())
    }

    fn rename_file(&self, path: &Path, filename: &str) -> Result<(), FmError> {
        let mut parent = PathBuf::from(path);
        parent.pop();
        std::fs::rename(path, parent.join(filename))?;
        Ok(())
    }
}
