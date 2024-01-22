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

struct BulkExecutor {
    original_filepath: Vec<PathBuf>,
    temp_file: PathBuf,
    new_filenames: Vec<String>,
    parent_dir: String,
}

impl BulkExecutor {
    fn new(original_filepath: Vec<PathBuf>, parent_dir: &str) -> Self {
        let temp_file = generate_random_filepath();
        Self {
            original_filepath,
            temp_file,
            new_filenames: vec![],
            parent_dir: parent_dir.to_owned(),
        }
    }

    fn ask_filenames(mut self, opener: &Opener) -> Result<Self> {
        create_random_file(&self.temp_file)?;
        log_info!("created {temp_file}", temp_file = self.temp_file.display());
        self.write_original_names()?;
        let original_modification = get_modified_date(&self.temp_file)?;
        open_temp_file_with_editor(&self.temp_file, opener)?;

        watch_modification_in_thread(&self.temp_file, original_modification)?;
        self.new_filenames = get_new_filenames(&self.temp_file)?;
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

    fn execute(&self) -> Result<(OptionVecPathBuf, OptionVecPathBuf)> {
        let paths = self.rename_create();
        self.del_temporary_file()?;
        paths
    }

    fn rename_create(&self) -> Result<(OptionVecPathBuf, OptionVecPathBuf)> {
        let renamed_paths = self.rename_all(&self.new_filenames)?;
        let created_paths = self.create_all_files(&self.new_filenames)?;
        Ok((renamed_paths, created_paths))
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

    fn create_all_files(&self, new_filenames: &[String]) -> Result<Option<Vec<PathBuf>>> {
        let mut paths = vec![];
        for filename in new_filenames.iter().skip(self.original_filepath.len()) {
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

    fn del_temporary_file(&self) -> Result<()> {
        std::fs::remove_file(&self.temp_file)?;
        Ok(())
    }
}

fn get_modified_date(filepath: &Path) -> Result<SystemTime> {
    Ok(std::fs::metadata(filepath)?.modified()?)
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

#[derive(Default)]
pub struct Bulk {
    bulk: Option<BulkExecutor>,
}

/// Bulk holds a `BulkExecutor` only when bulk actionmap, `None` otherwise.
///
/// Once `ask_filenames` is executed, a new tmp file is created. It's filled with every filename
/// of flagged files in current directory.
/// Modifications of this file are watched in a separate thread.
/// Once the file is written, its content is parsed and a confirmation is asked : `format_confirmation`
/// Renaming or creating is execute in bulk with `execute`.
impl Bulk {
    /// Reset bulk content to None, droping all created or renomed filename from previous execution.
    pub fn reset(&mut self) {
        self.bulk = None;
    }

    /// Ask user for filename.
    ///
    /// Creates a temp file with every flagged filename in current dir.
    /// Modification of this file are then watched in a thread.
    /// The mode will change to BulkAction once something is written in the file.
    ///
    /// # Errors
    ///
    /// May fail if the user can't create a file, read or write in /tmp
    /// May also fail if the watching thread fail.
    pub fn ask_filenames(
        &mut self,
        flagged_in_current_dir: Vec<PathBuf>,
        current_tab_path_str: &str,
        opener: &Opener,
    ) -> Result<()> {
        self.bulk = Some(
            BulkExecutor::new(flagged_in_current_dir, current_tab_path_str)
                .ask_filenames(opener)?,
        );
        Ok(())
    }

    /// String representation of the filetree modifications.
    pub fn format_confirmation(&self) -> Vec<String> {
        if let Some(bulk) = &self.bulk {
            let mut lines: Vec<String> = bulk
                .original_filepath
                .iter()
                .zip(bulk.new_filenames.iter())
                .map(|(original, new)| {
                    format!("RENAME: {original} -> {new}", original = original.display())
                })
                .collect();
            for new in bulk.new_filenames.iter().skip(bulk.original_filepath.len()) {
                lines.push(format!("CREATE: {new}"));
            }
            lines
        } else {
            vec![]
        }
    }

    /// Execute the action parsed from the file.
    ///
    /// # Errors
    ///
    /// May fail if bulk is still set to None. It should never happen.
    /// May fail if the new file can't be created or the flagged file can't be renamed.
    pub fn execute(&mut self) -> Result<(OptionVecPathBuf, OptionVecPathBuf)> {
        let Some(bulk) = &mut self.bulk else {
            return Err(anyhow!("bulk shouldn't be None"));
        };
        let ret = bulk.execute();
        self.reset();
        ret
    }
}

type OptionVecPathBuf = Option<Vec<PathBuf>>;
