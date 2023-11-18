use std::fs::{create_dir, read_dir, remove_dir_all};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::{Local, NaiveDateTime};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::common::read_lines;
use crate::common::{TRASH_FOLDER_FILES, TRASH_FOLDER_INFO, TRASH_INFO_EXTENSION};
use crate::impl_selectable_content;
use crate::log_info;
use crate::log_line;

const TRASHINFO_DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

/// Holds the information about a trashed file.
/// Follow the specifications of .trashinfo files as described in
/// [Trash freedesktop specs](https://specifications.freedesktop.org/trash-spec/trashspec-latest.html)
/// It knows
/// - where the file came from,
/// - what name it was given when trashed,
/// - when it was trashed
#[derive(Debug, Clone)]
pub struct TrashInfo {
    origin: PathBuf,
    dest_name: String,
    deletion_date: String,
}

impl TrashInfo {
    /// Returns a new `TrashInfo` instance.
    /// The deletion_date is calculated on creation, before the file is actually trashed.
    pub fn new(origin: &Path, dest_name: &str) -> Self {
        let date = Local::now();
        let deletion_date = format!("{}", date.format(TRASHINFO_DATETIME_FORMAT));
        let dest_name = dest_name.to_owned();
        Self {
            origin: PathBuf::from(origin),
            dest_name,
            deletion_date,
        }
    }

    fn to_string(&self) -> Result<String> {
        Ok(format!(
            "[Trash Info]
Path={origin}
DeletionDate={date}
",
            origin = url_escape::encode_fragment(&self.origin.to_string_lossy()),
            date = self.deletion_date
        ))
    }

    /// Write itself into a .trashinfo file.
    /// The format looks like :
    ///
    /// [TrashInfo]
    /// Path=/home/quentin/Documents
    /// DeletionDate=2022-12-31T22:45:55
    pub fn write_trash_info(&self, dest: &Path) -> Result<()> {
        log_info!("writing trash_info {} for {:?}", self, dest);

        let mut file = std::fs::File::create(dest)?;
        if let Err(e) = write!(file, "{}", self.to_string()?) {
            log_info!("Couldn't write to trash file: {}", e)
        }
        Ok(())
    }

    /// Reads a .trashinfo file and parse it into a new instance.
    ///
    /// Let say `Documents.trashinfo` contains :
    ///
    /// ```not_rust
    /// [TrashInfo]
    /// Path=/home/quentin/Documents
    /// DeletionDate=2022-12-31T22:45:55
    /// ```
    ///
    /// It will be parsed into
    /// ```rust
    /// TrashInfo { PathBuf::from("/home/quentin/Documents"), "Documents", "2022-12-31T22:45:55" }
    /// ```
    pub fn from_trash_info_file(trash_info_file: &Path) -> Result<Self> {
        let (option_path, option_deleted_time) = Self::parse_trash_info_file(trash_info_file)?;

        match (option_path, option_deleted_time) {
            (Some(origin), Some(deletion_date)) => {
                let dest_name = Self::get_dest_name(trash_info_file)?;
                Ok(Self {
                    dest_name,
                    deletion_date,
                    origin,
                })
            }
            _ => Err(anyhow!("Couldn't parse the trash info file")),
        }
    }

    fn get_dest_name(trash_info_file: &Path) -> Result<String> {
        if let Some(dest_name) = trash_info_file.file_name() {
            let dest_name = Self::remove_extension(dest_name.to_str().unwrap().to_owned())?;
            Ok(dest_name)
        } else {
            Err(anyhow!("Couldn't parse the trash info filename"))
        }
    }

    fn parse_trash_info_file(trash_info_file: &Path) -> Result<(Option<PathBuf>, Option<String>)> {
        let mut option_path: Option<PathBuf> = None;
        let mut option_deleted_time: Option<String> = None;

        if let Ok(mut lines) = read_lines(trash_info_file) {
            let Some(Ok(first_line)) = lines.next() else {
                return Err(anyhow!("Unreadable TrashInfo file"));
            };
            if !first_line.starts_with("[Trash Info]") {
                return Err(anyhow!("First line should start with [TrashInfo]"));
            }

            for line in lines {
                let Ok(line) = line else {
                    continue;
                };
                if option_path.is_none() && line.starts_with("Path=") {
                    option_path = Some(Self::parse_option_path(&line));
                    continue;
                }
                if option_deleted_time.is_none() && line.starts_with("DeletionDate=") {
                    option_deleted_time = Some(Self::parse_deletion_date(&line)?);
                }
            }
        }

        Ok((option_path, option_deleted_time))
    }

    fn parse_option_path(line: &str) -> PathBuf {
        let path_part = &line[5..];
        let cow_path_str = url_escape::decode(path_part);
        let path_str = cow_path_str.as_ref();
        PathBuf::from(path_str)
    }

    fn parse_deletion_date(line: &str) -> Result<String> {
        let deletion_date_str = &line[13..];
        match parsed_date_from_path_info(deletion_date_str) {
            Ok(()) => Ok(deletion_date_str.to_owned()),
            Err(e) => Err(e),
        }
    }

    fn remove_extension(mut destname: String) -> Result<String> {
        if destname.ends_with(TRASH_INFO_EXTENSION) {
            destname.truncate(destname.len() - 10);
            Ok(destname)
        } else {
            Err(anyhow!(
                "trahsinfo: filename doesn't contain {TRASH_INFO_EXTENSION}"
            ))
        }
    }
}

impl std::fmt::Display for TrashInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} - trashed on {}",
            &self.origin.display(),
            self.deletion_date
        )
    }
}

/// Represent a view of the trash.
/// Its content is navigable so we use a Vector to hold the content.
/// Only files that share the same mount point as the trash folder (generally ~/.local/share/Trash)
/// can be moved to trash.
/// Other files are unaffected.
#[derive(Clone)]
pub struct Trash {
    /// Trashed files info.
    content: Vec<TrashInfo>,
    index: usize,
    /// The path to the trashed files
    pub trash_folder_files: String,
    trash_folder_info: String,
}

impl Trash {
    fn pick_dest_name(&self, origin: &Path) -> Result<String> {
        if let Some(file_name) = origin.file_name() {
            let mut dest = file_name
                .to_str()
                .context("pick_dest_name: Couldn't parse the origin filename into a string")?
                .to_owned();
            let mut dest_path = PathBuf::from(&self.trash_folder_files);
            dest_path.push(&dest);
            while dest_path.exists() {
                dest.push_str(&rand_string());
                dest_path = PathBuf::from(&self.trash_folder_files);
                dest_path.push(&dest);
            }
            return Ok(dest);
        }
        Err(anyhow!("pick_dest_name: Couldn't extract the filename",))
    }

    /// Creates an empty view of the trash.
    /// No file is read here, we wait for the user to open the trash first.
    pub fn new() -> Result<Self> {
        let trash_folder_files = shellexpand::tilde(TRASH_FOLDER_FILES).to_string();
        let trash_folder_info = shellexpand::tilde(TRASH_FOLDER_INFO).to_string();
        create_if_not_exists(&trash_folder_files)?;
        create_if_not_exists(&trash_folder_info)?;

        let index = 0;
        let content = vec![];

        Ok(Self {
            content,
            index,
            trash_folder_files,
            trash_folder_info,
        })
    }

    fn parse_updated_content(trash_folder_info: &str) -> Result<Vec<TrashInfo>> {
        match read_dir(trash_folder_info) {
            Ok(read_dir) => {
                let content: Vec<TrashInfo> = read_dir
                    .filter_map(|res_direntry| res_direntry.ok())
                    .filter(|direntry| direntry.path().extension().is_some())
                    .filter(|direntry| {
                        direntry.path().extension().unwrap().to_str().unwrap() == "trashinfo"
                    })
                    .map(|direntry| TrashInfo::from_trash_info_file(&direntry.path()))
                    .filter_map(|trashinfo_res| trashinfo_res.ok())
                    .collect();

                Ok(content)
            }
            Err(error) => {
                log_info!("Couldn't read path {:?} - {}", trash_folder_info, error);
                Err(anyhow!(error))
            }
        }
    }

    /// Parse the info files into a new instance.
    /// Only the file we can parse are read.
    pub fn update(&mut self) -> Result<()> {
        self.index = 0;
        self.content = Self::parse_updated_content(&self.trash_folder_info)?;
        Ok(())
    }

    /// Move a file to the trash folder and create a new trash info file.
    /// Add a new TrashInfo to the content.
    pub fn trash(&mut self, origin: &Path) -> Result<()> {
        if origin.is_relative() {
            return Err(anyhow!("trash: origin path should be absolute"));
        }

        let dest_file_name = self.pick_dest_name(origin)?;

        self.execute_trash(TrashInfo::new(origin, &dest_file_name), &dest_file_name)
    }

    fn concat_path(root: &str, filename: &str) -> PathBuf {
        let mut concatened_path = PathBuf::from(root);
        concatened_path.push(filename);
        concatened_path
    }

    fn trashfile_path(&self, dest_file_name: &str) -> PathBuf {
        Self::concat_path(&self.trash_folder_files, dest_file_name)
    }

    fn trashinfo_path(&self, dest_trashinfo_name: &str) -> PathBuf {
        let mut dest_trashinfo_name = dest_trashinfo_name.to_owned();
        dest_trashinfo_name.push_str(TRASH_INFO_EXTENSION);
        Self::concat_path(&self.trash_folder_info, &dest_trashinfo_name)
    }

    fn execute_trash(&mut self, trash_info: TrashInfo, dest_file_name: &str) -> Result<()> {
        let trashfile_filename = &self.trashfile_path(dest_file_name);
        match std::fs::rename(&trash_info.origin, trashfile_filename) {
            Err(error) => {
                log_info!("Couldn't trash {trash_info}. Error: {error:?}");
                Ok(())
            }
            Ok(()) => {
                Self::log_trash_add(&trash_info.origin, dest_file_name);
                trash_info.write_trash_info(&self.trashinfo_path(dest_file_name))?;
                self.content.push(trash_info);
                Ok(())
            }
        }
    }

    fn log_trash_add(origin: &Path, dest_file_name: &str) {
        log_info!("moved to trash {:?} -> {:?}", origin, dest_file_name);
        log_line!("moved to trash {:?} -> {:?}", origin, dest_file_name);
    }

    /// Empty the trash, removing all the files and the trashinfo.
    /// This action requires a confirmation.
    /// Watchout, it may delete files that weren't parsed.
    pub fn empty_trash(&mut self) -> Result<()> {
        self.empty_trash_dirs()?;
        let number_of_elements = self.content.len();
        self.content = vec![];
        Self::log_trash_empty(number_of_elements);
        Ok(())
    }

    fn empty_trash_dirs(&self) -> Result<(), std::io::Error> {
        Self::empty_dir(&self.trash_folder_files)?;
        Self::empty_dir(&self.trash_folder_info)
    }

    fn empty_dir(dir: &str) -> Result<(), std::io::Error> {
        remove_dir_all(dir)?;
        create_dir(dir)
    }

    fn log_trash_empty(number_of_elements: usize) {
        log_line!("Emptied the trash: {number_of_elements} files permanently deleted");
        log_info!("Emptied the trash: {number_of_elements} files permanently deleted");
    }

    fn remove_selected_file(&mut self) -> Result<(PathBuf, PathBuf, PathBuf)> {
        if self.is_empty() {
            return Err(anyhow!(
                "remove selected file: Can't restore from an empty trash",
            ));
        }
        let trashinfo = &self.content[self.index];
        let origin = trashinfo.origin.to_owned();

        let parent = find_parent(&trashinfo.origin)?;

        let trashed_file_content = self.trashfile_path(&trashinfo.dest_name);
        let trashed_file_info = self.trashinfo_path(&trashinfo.dest_name);

        if !trashed_file_content.exists() {
            return Err(anyhow!("trash restore: Couldn't find the trashed file",));
        }

        if !trashed_file_info.exists() {
            return Err(anyhow!("trash restore: Couldn't find the trashed info",));
        }

        self.remove_from_content_and_delete_trashinfo(&trashed_file_info)?;

        Ok((origin, trashed_file_content, parent))
    }

    fn remove_from_content_and_delete_trashinfo(&mut self, trashed_file_info: &Path) -> Result<()> {
        self.content.remove(self.index);
        std::fs::remove_file(trashed_file_info)?;
        Ok(())
    }

    /// Restores a file from the trash to its previous directory.
    /// If the parent (or ancestor) folder were deleted, it is recreated.
    pub fn restore(&mut self) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }
        let (origin, trashed_file_content, parent) = self.remove_selected_file()?;
        Self::execute_restore(&origin, &trashed_file_content, &parent)?;
        Self::log_trash_restore(&origin);
        Ok(())
    }

    fn execute_restore(origin: &Path, trashed_file_content: &Path, parent: &Path) -> Result<()> {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?
        }
        std::fs::rename(trashed_file_content, origin)?;
        Ok(())
    }

    fn log_trash_restore(origin: &Path) {
        log_line!("Trash restored: {origin}", origin = origin.display());
    }

    /// Deletes a file permanently from the trash.
    pub fn delete_permanently(&mut self) -> Result<()> {
        if self.is_empty() {
            return Ok(());
        }

        let (_, trashed_file_content, _) = self.remove_selected_file()?;

        std::fs::remove_file(&trashed_file_content)?;
        Self::log_trash_remove(&trashed_file_content);

        if self.index > 0 {
            self.index -= 1
        }
        Ok(())
    }

    fn log_trash_remove(trashed_file_content: &Path) {
        log_line!(
            "Trash removed: {trashed_file_content}",
            trashed_file_content = trashed_file_content.display()
        );
    }
}

impl_selectable_content!(TrashInfo, Trash);

fn parsed_date_from_path_info(ds: &str) -> Result<()> {
    NaiveDateTime::parse_from_str(ds, TRASHINFO_DATETIME_FORMAT)?;
    Ok(())
}

fn rand_string() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(2)
        .map(char::from)
        .collect()
}

fn find_parent(path: &Path) -> Result<PathBuf> {
    Ok(path
        .parent()
        .ok_or_else(|| anyhow!("find_parent_as_string : Couldn't find parent of {path:?}"))?
        .to_owned())
}

fn create_if_not_exists<P>(path: P) -> std::io::Result<()>
where
    std::path::PathBuf: From<P>,
    P: std::convert::AsRef<std::path::Path> + std::marker::Copy,
{
    if !std::path::PathBuf::from(path).exists() {
        std::fs::create_dir_all(path)?
    }
    Ok(())
}
