use rand::Rng;
use std::io::{self, BufRead};
use std::path::PathBuf;

use crate::opener::{ExtensionKind, Opener};

//TODO: comme pour skim, attach a Arc<Term> and use it to display the editor

static TMP_FOLDER: &str = "/tmp";

struct BulkRenamer {
    original_filepath: Vec<PathBuf>,
    temp_file: PathBuf,
}

impl BulkRenamer {
    pub fn new(original_filepath: Vec<PathBuf>) -> Result<Self, io::Error> {
        let temp_file = Self::create_random_file()?;
        Ok(Self {
            original_filepath,
            temp_file,
        })
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

    fn create_random_file() -> Result<PathBuf, io::Error> {
        let mut filepath = PathBuf::from(&TMP_FOLDER);
        filepath.push(Self::random_name());
        let _ = std::fs::File::create(&filepath)?;
        Ok(filepath)
    }

    fn write_original_names(&self) -> Result<(), io::Error> {
        let filepath = &self.temp_file;
        for path in self.original_filepath.iter() {
            if let Some(os_filename) = path.file_name() {
                if let Some(filename) = os_filename.to_str() {
                    std::fs::write(&filepath, filename)?
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
        filepath: PathBuf,
        original_modification: std::time::SystemTime,
    ) -> Result<bool, io::Error> {
        let last_modification = std::fs::metadata(&filepath)?.modified()?;
        Ok(last_modification <= original_modification)
    }

    fn get_new_filenames(&self, filepath: PathBuf) -> Result<Vec<String>, io::Error> {
        let file = std::fs::File::open(&filepath)?;

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
        Ok(std::fs::remove_file(&filepath)?)
    }

    fn rename_all(&self, new_filenames: Vec<String>) -> Result<(), io::Error> {
        for (path, filename) in self.original_filepath.iter().zip(new_filenames.iter()) {
            self.rename(path, filename)?
        }
        Ok(())
    }

    fn rename(&self, path: &PathBuf, filename: &str) -> Result<(), io::Error> {
        let mut parent = path.clone();
        parent.pop();
        Ok(std::fs::rename(path, parent.join(&filename))?)
    }
}
