use rand::Rng;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime};

use crate::opener::{ExtensionKind, Opener};

static TMP_FOLDER: &str = "/tmp";

pub struct Bulkrename {
    original_filepath: Vec<PathBuf>,
    temp_file: PathBuf,
}

impl Bulkrename {
    pub fn new(original_filepath: Vec<PathBuf>) -> Result<Self, io::Error> {
        let temp_file = Self::generate_random_filepath()?;
        Ok(Self {
            original_filepath,
            temp_file,
        })
    }

    pub fn rename(&mut self, opener: &Opener) -> Result<(), io::Error> {
        self.write_original_names()?;
        let original_modification = Self::get_modified_date(&self.temp_file)?;
        self.open_temp_file_with_editor(opener)?;

        Self::watch_modification_in_thread(self.temp_file.clone(), original_modification);

        self.rename_all(self.get_new_filenames()?)?;
        self.delete_temp_file()?;
        Ok(())
    }

    fn watch_modification_in_thread(filepath: PathBuf, original_modification: SystemTime) {
        let handle = thread::spawn(move || loop {
            if Self::is_file_modified(&filepath, original_modification).unwrap_or(true) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        });
        handle.join().unwrap();
    }

    fn get_modified_date(filepath: &PathBuf) -> Result<SystemTime, io::Error> {
        std::fs::metadata(filepath)?.modified()
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

    fn generate_random_filepath() -> Result<PathBuf, io::Error> {
        let mut filepath = PathBuf::from(&TMP_FOLDER);
        filepath.push(Self::random_name());
        Ok(filepath)
    }

    fn write_original_names(&self) -> Result<(), io::Error> {
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

    fn open_temp_file_with_editor(&self, opener: &Opener) -> Result<(), io::Error> {
        let filepath = &self.temp_file;
        if let Some(editor_info) = opener.get(ExtensionKind::Text) {
            opener.open_with(editor_info, filepath.to_owned())
        };
        Ok(())
    }

    fn is_file_modified(
        path: &PathBuf,
        original_modification: std::time::SystemTime,
    ) -> Result<bool, io::Error> {
        let last_modification = Self::get_modified_date(path)?;
        Ok(last_modification > original_modification)
    }

    fn get_new_filenames(&self) -> Result<Vec<String>, io::Error> {
        let file = std::fs::File::open(&self.temp_file)?;

        let reader = std::io::BufReader::new(file);
        let mut new_names = vec![];
        for line in reader.lines() {
            let line2 = line?;
            let line = line2.trim();
            if line.is_empty() {
                return Err(io::Error::new(io::ErrorKind::Other, "empty filename"));
            }
            new_names.push(line2);
        }
        if new_names.len() < self.original_filepath.len() {
            return Err(io::Error::new(io::ErrorKind::Other, "not enough filenames"));
        }
        Ok(new_names)
    }

    fn delete_temp_file(&self) -> Result<(), io::Error> {
        let filepath = &self.temp_file;
        std::fs::remove_file(&filepath)
    }

    fn rename_all(&self, new_filenames: Vec<String>) -> Result<(), io::Error> {
        for (path, filename) in self.original_filepath.iter().zip(new_filenames.iter()) {
            self.rename_file(path, filename)?
        }
        Ok(())
    }

    fn rename_file(&self, path: &PathBuf, filename: &str) -> Result<(), io::Error> {
        let mut parent = path.clone();
        parent.pop();
        std::fs::rename(path, parent.join(&filename))
    }
}
