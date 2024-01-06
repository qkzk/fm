use anyhow::{anyhow, Result};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use crate::common::TMP_FOLDER_PATH;
use crate::common::{random_name, rename};
use crate::io::Opener;
use crate::log_info;
use crate::log_line;
use crate::modes::mode::BulkAction;
use crate::{impl_content, impl_selectable};

trait DelTemp {
    fn del_temporary_file(&self, temp_file: &Path) -> Result<()> {
        std::fs::remove_file(temp_file)?;
        Ok(())
    }
}

trait BulkExecute {
    fn execute(&self) -> Result<Option<Vec<PathBuf>>>;
}

struct Renamer {
    original_filepath: Vec<PathBuf>,
    temp_file: PathBuf,
    new_filenames: Vec<String>,
}

impl DelTemp for Renamer {}

impl BulkExecute for Renamer {
    fn execute(&self) -> Result<Option<Vec<PathBuf>>> {
        let paths = self.rename_all(&self.new_filenames);
        self.del_temporary_file(&self.temp_file)?;
        paths
    }
}

impl Renamer {
    /// Creates a new renamer
    fn new(original_filepath: Vec<PathBuf>) -> Self {
        let temp_file = generate_random_filepath();
        Self {
            original_filepath,
            temp_file,
            new_filenames: vec![],
        }
    }

    /// Rename the files.
    /// The tempory file is opened with our `Opener` crate, allowing us
    /// to use the default text file editor.
    /// Filenames are sanitized before processing.
    fn ask_filenames(mut self, opener: &Opener) -> Result<Self> {
        self.write_original_names()?;
        let original_modification = get_modified_date(&self.temp_file)?;
        open_temp_file_with_editor(&self.temp_file, opener)?;

        watch_modification_in_thread(&self.temp_file, original_modification)?;

        let new_filenames = get_new_filenames(&self.temp_file)?;

        if new_filenames.len() < self.original_filepath.len() {
            std::fs::remove_file(&self.temp_file)?;
            return Err(anyhow!("new filenames: not enough filenames"));
        }
        self.new_filenames = new_filenames;
        Ok(self)
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

    fn rename_all(&self, new_filenames: &[String]) -> Result<Option<Vec<PathBuf>>> {
        let mut paths = vec![];
        for (path, filename) in self.original_filepath.iter().zip(new_filenames.iter()) {
            match rename(path, filename) {
                Ok(path) => paths.push(path),
                Err(error) => log_info!(
                    "Error renaming {path} to {filename}. Error: {error:?}",
                    path = path.display()
                ),
            }
        }
        log_line!("Bulk renamed {len} files", len = paths.len());
        Ok(Some(paths))
    }
}

struct Creator {
    parent_dir: String,
    temp_file: PathBuf,
    new_filenames: Vec<String>,
}

impl DelTemp for Creator {}

impl BulkExecute for Creator {
    fn execute(&self) -> Result<Option<Vec<PathBuf>>> {
        let paths = self.create_all_files(&self.new_filenames)?;
        self.del_temporary_file(&self.temp_file)?;
        Ok(paths)
    }
}

impl Creator {
    fn new(path_str: &str) -> Self {
        let temp_file = generate_random_filepath();
        Self {
            parent_dir: path_str.to_owned(),
            temp_file,
            new_filenames: vec![],
        }
    }

    fn ask_filenames(mut self, opener: &Opener) -> Result<Self> {
        create_random_file(&self.temp_file)?;
        log_info!("created {temp_file}", temp_file = self.temp_file.display());
        let original_modification = get_modified_date(&self.temp_file)?;
        open_temp_file_with_editor(&self.temp_file, opener)?;

        watch_modification_in_thread(&self.temp_file, original_modification)?;
        self.new_filenames = get_new_filenames(&self.temp_file)?;
        Ok(self)
    }

    fn create_all_files(&self, new_filenames: &[String]) -> Result<Option<Vec<PathBuf>>> {
        let mut paths = vec![];
        for filename in new_filenames.iter() {
            let Some(path) = self.create_file(filename)? else {
                continue;
            };
            paths.push(path)
        }
        log_line!("Bulk created {len} files", len = paths.len());
        Ok(Some(paths))
    }

    fn create_file(&self, filename: &str) -> Result<Option<PathBuf>> {
        let mut new_path = std::path::PathBuf::from(&self.parent_dir);
        if !filename.ends_with('/') {
            new_path.push(filename);
            let Some(parent) = new_path.parent() else {
                return Ok(None);
            };
            log_info!("Bulk new files. Creating parent: {}", parent.display());
            if std::fs::create_dir_all(parent).is_err() {
                return Ok(None);
            };
            log_info!("creating: {new_path:?}");
            std::fs::File::create(&new_path)?;
            log_line!("Bulk created {new_path}", new_path = new_path.display());
        } else {
            new_path.push(filename);
            log_info!("Bulk creating dir: {}", new_path.display());
            std::fs::create_dir_all(&new_path)?;
            log_line!("Bulk created {new_path}", new_path = new_path.display());
        }
        Ok(Some(new_path))
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
    renamer: Option<Renamer>,
    creator: Option<Creator>,
}

impl Default for Bulk {
    fn default() -> Self {
        Self {
            content: vec![
                "Rename files".to_string(),
                "New files or folders. End folder with a slash '/'".to_string(),
            ],
            index: 0,
            renamer: None,
            creator: None,
        }
    }
}

impl Bulk {
    /// True if Bulk is set to rename files.
    /// False if Bulk is set to create files.
    pub fn is_rename(&self) -> bool {
        self.index == 0
    }

    pub fn bulk_mode(&self) -> BulkAction {
        if self.is_rename() {
            BulkAction::Rename
        } else {
            BulkAction::Create
        }
    }

    /// Reset to default values.
    pub fn reset(&mut self) {
        self.renamer = None;
        self.creator = None;
        self.index = 0;
    }

    /// Ask the new filenames
    ///
    /// Both may fail if the current user can't write in /tmp, since
    /// they create a temporary file.
    pub fn ask_filenames(
        &mut self,
        flagged_in_current_dir: Vec<PathBuf>,
        current_tab_path_str: &str,
        opener: &Opener,
    ) -> Result<BulkAction> {
        if self.is_rename() {
            self.renamer = Some(Renamer::new(flagged_in_current_dir).ask_filenames(opener)?);
        } else {
            self.creator = Some(Creator::new(current_tab_path_str).ask_filenames(opener)?);
        }
        Ok(self.bulk_mode())
    }

    /// Execute the selected bulk method depending on the index.
    /// First method is a rename of selected files,
    /// Second is the creation of files or folders,
    ///
    /// # Errors
    ///
    /// renamer may fail if we can't rename a file (permissions...)
    /// creator may fail if we can't write in current directory.
    pub fn execute(&mut self) -> Result<Option<Vec<PathBuf>>> {
        log_info!("bulk execute: {action:?}", action = self.bulk_mode());
        let paths = if self.is_rename() {
            let Some(renamer) = &mut self.renamer else {
                return Ok(None);
            };
            renamer.execute()?
        } else {
            let Some(creator) = &mut self.creator else {
                return Ok(None);
            };
            creator.execute()?
        };
        self.reset();
        Ok(paths)
    }

    pub fn format_confirmation(&self) -> Vec<String> {
        if let Some(renamer) = &self.renamer {
            renamer
                .original_filepath
                .iter()
                .zip(renamer.new_filenames.iter())
                .map(|(original, new)| {
                    format!("{original} -> {new}", original = original.display())
                })
                .collect()
        } else if let Some(creator) = &self.creator {
            creator.new_filenames.clone()
        } else {
            vec![]
        }
    }
}

// impl_selectable_content!(String, Bulk);
impl_selectable!(Bulk);
impl_content!(String, Bulk);
