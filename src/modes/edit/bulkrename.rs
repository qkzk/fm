use anyhow::{anyhow, Result};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::app::Status;
use crate::common::random_name;
use crate::common::TMP_FOLDER_PATH;
use crate::io::Opener;
use crate::log_line;
use crate::{impl_selectable_content, log_info};

struct Renamer<'a> {
    original_filepath: Vec<&'a Path>,
    temp_file: PathBuf,
}

impl<'a> Renamer<'a> {
    /// Creates a new renamer
    fn new(original_filepath: Vec<&'a Path>) -> Self {
        let temp_file = generate_random_filepath();
        Self {
            original_filepath,
            temp_file,
        }
    }

    /// Rename the files.
    /// The tempory file is opened with our `Opener` crate, allowing us
    /// to use the default text file editor.
    /// Filenames are sanitized before processing.
    fn rename(&mut self, opener: &Opener) -> Result<()> {
        self.write_original_names()?;
        let original_modification = get_modified_date(&self.temp_file)?;
        open_temp_file_with_editor(&self.temp_file, opener)?;

        watch_modification_in_thread(&self.temp_file, original_modification)?;

        let new_filenames = get_new_filenames(&self.temp_file)?;

        if new_filenames.len() < self.original_filepath.len() {
            std::fs::remove_file(&self.temp_file)?;
            return Err(anyhow!("new filenames: not enough filenames"));
        }

        self.rename_all(new_filenames)?;
        std::fs::remove_file(&self.temp_file)?;
        Ok(())
    }

    fn write_original_names(&self) -> Result<()> {
        let mut file = std::fs::File::create(&self.temp_file)?;
        log_info!("created {temp_file}", temp_file = self.temp_file.display());

        for path in &self.original_filepath {
            let Some(os_filename) = path.file_name() else {
                return Ok(());
            };
            let Some(filename) = os_filename.to_str() else {
                return Ok(());
            };
            file.write_all(filename.as_bytes())?;
            file.write_all(&[b'\n'])?;
        }
        Ok(())
    }

    fn rename_all(&self, new_filenames: Vec<String>) -> Result<()> {
        let mut counter = 0;
        for (path, filename) in self.original_filepath.iter().zip(new_filenames.iter()) {
            let new_name = sanitize_filename::sanitize(filename);
            self.rename_file(path, &new_name)?;
            counter += 1;
            log_line!("Bulk renamed {path} to {new_name}", path = path.display());
        }
        log_line!("Bulk renamed {counter} files");
        Ok(())
    }

    fn rename_file(&self, path: &Path, filename: &str) -> Result<()> {
        let mut parent = PathBuf::from(path);
        parent.pop();
        std::fs::rename(path, parent.join(filename))?;
        Ok(())
    }
}

struct Creator<'a> {
    parent_dir: &'a str,
    temp_file: PathBuf,
}

impl<'a> Creator<'a> {
    fn new(path_str: &'a str) -> Self {
        let temp_file = generate_random_filepath();
        Self {
            parent_dir: path_str,
            temp_file,
        }
    }

    fn create_files(&mut self, opener: &Opener) -> Result<()> {
        create_random_file(&self.temp_file)?;
        log_info!("created {temp_file}", temp_file = self.temp_file.display());
        let original_modification = get_modified_date(&self.temp_file)?;
        open_temp_file_with_editor(&self.temp_file, opener)?;

        watch_modification_in_thread(&self.temp_file, original_modification)?;

        self.create_all_files(&get_new_filenames(&self.temp_file)?)?;
        std::fs::remove_file(&self.temp_file)?;
        Ok(())
    }

    fn create_all_files(&self, new_filenames: &[String]) -> Result<()> {
        let mut counter = 0;
        for filename in new_filenames.iter() {
            let mut new_path = std::path::PathBuf::from(self.parent_dir);
            if !filename.ends_with('/') {
                new_path.push(filename);
                let Some(parent) = new_path.parent() else {
                    return Ok(());
                };
                log_info!("Bulk new files. Creating parent: {}", parent.display());
                if std::fs::create_dir_all(parent).is_err() {
                    continue;
                };
                log_info!("creating: {new_path:?}");
                std::fs::File::create(&new_path)?;
                log_line!("Bulk created {new_path}", new_path = new_path.display());
                counter += 1;
            } else {
                new_path.push(filename);
                log_info!("Bulk creating dir: {}", new_path.display());
                std::fs::create_dir_all(&new_path)?;
                log_line!("Bulk created {new_path}", new_path = new_path.display());
                counter += 1;
            }
        }
        log_line!("Bulk created {counter} files");
        Ok(())
    }
}

fn generate_random_filepath() -> PathBuf {
    let mut filepath = PathBuf::from(&TMP_FOLDER_PATH);
    filepath.push(random_name());
    filepath
}

fn watch_modification_in_thread(filepath: &Path, original_modification: SystemTime) -> Result<()> {
    let filepath = filepath.to_owned();
    let handle = thread::spawn(move || loop {
        if is_file_modified(&filepath, original_modification).unwrap_or(true) {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    });
    match handle.join() {
        Ok(handle) => Ok(handle),
        Err(e) => Err(anyhow!("watch thread failed {e:?}")),
    }
}

fn get_modified_date(filepath: &Path) -> Result<SystemTime> {
    Ok(std::fs::metadata(filepath)?.modified()?)
}

fn create_random_file(temp_file: &Path) -> Result<()> {
    std::fs::File::create(temp_file)?;
    Ok(())
}

fn open_temp_file_with_editor(temp_file: &Path, opener: &Opener) -> Result<()> {
    log_info!("opening tempory file {:?}", temp_file);
    opener.open_single(temp_file)
}

fn is_file_modified(path: &Path, original_modification: std::time::SystemTime) -> Result<bool> {
    Ok(get_modified_date(path)? > original_modification)
}

fn get_new_filenames(temp_file: &Path) -> Result<Vec<String>> {
    let file = std::fs::File::open(temp_file)?;
    let reader = std::io::BufReader::new(file);

    let new_names: Vec<String> = reader
        .lines()
        .flatten()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect();
    Ok(new_names)
}

pub struct Bulk {
    pub content: Vec<String>,
    index: usize,
}

impl Default for Bulk {
    fn default() -> Self {
        Self {
            content: vec![
                "Rename files".to_string(),
                "New files or folders. End folder with a slash '/'".to_string(),
            ],
            index: 0,
        }
    }
}

impl Bulk {
    /// Execute the selected bulk method depending on the index.
    /// First method is a rename of selected files,
    /// Second is the creation of files or folders,
    ///
    /// # Errors
    ///
    /// renamer may fail if we can't rename a file (permissions...)
    /// creator may fail if we can't write in current directory.
    /// Both may fail if the current user can't write in /tmp, since
    /// they create a temporary file.
    pub fn execute_bulk(&self, status: &Status) -> Result<()> {
        match self.index {
            0 => Renamer::new(status.flagged_in_current_dir())
                .rename(&status.internal_settings.opener)?,
            1 => Creator::new(status.current_tab_path_str())
                .create_files(&status.internal_settings.opener)?,
            _ => (),
        };
        Ok(())
    }
}

impl_selectable_content!(String, Bulk);
