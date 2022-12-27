use std::fs::{create_dir, read_dir, remove_dir_all};
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime};
use log::info;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::constant_strings_paths::{TRASH_FOLDER_FILES, TRASH_FOLDER_INFO};
use crate::fm_error::{FmError, FmResult};
use crate::impl_selectable_content;
use crate::utils::read_lines;

static TRASHINFO_DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S";

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

    fn to_string(&self) -> FmResult<String> {
        Ok(format!(
            "[Trash Info]
Path={}
DeletionDate={}
",
            url_escape::encode_fragment(path_to_string(&self.origin)?),
            self.deletion_date
        ))
    }

    /// Write itself into a .trashinfo file.
    /// The format looks like :
    ///
    /// [TrashInfo]
    /// Path=/home/quentin/Documents
    /// DeletionDate=2022-12-31T22:45:55
    pub fn write_trash_info(&self, dest: &Path) -> FmResult<()> {
        info!("writing trash_info {} for {:?}", self, dest);

        // let mut file = OpenOptions::new().write(true).open(dest)?;
        let mut file = std::fs::File::create(dest)?;
        if let Err(e) = write!(file, "{}", self.to_string()?) {
            info!("Couldn't write to trash file: {}", e)
        }
        Ok(())
    }

    // TODO! manage non ascii bytes
    /// Reads a .trashinfo file and parse it into a new instance.
    /// ATM some non bytes chars allowed in path aren't supported.
    ///
    /// Let say `Document.trashinfo` contains :
    ///
    /// [TrashInfo]
    /// Path=/home/quentin/Documents
    /// DeletionDate=2022-12-31T22:45:55
    ///
    /// It will be parsed into
    /// TrashInfo { PathBuf::from("/home/quentin/Documents"), "Documents", "2022-12-31T22:45:55" }
    pub fn from_trash_info_file(trash_info_file: &Path) -> FmResult<Self> {
        let mut option_path: Option<PathBuf> = None;
        let mut option_deleted_time: Option<String> = None;
        let mut found_trash_info_line: bool = false;
        if let Some(dest_name) = trash_info_file.file_name() {
            let dest_name = Self::remove_extension(dest_name.to_str().unwrap().to_owned())?;
            if let Ok(lines) = read_lines(trash_info_file) {
                for (index, line_result) in lines.enumerate() {
                    if let Ok(line) = line_result.as_ref() {
                        if line.starts_with("[Trash Info]") {
                            if index == 0 {
                                found_trash_info_line = true;
                                continue;
                            } else {
                                return trashinfo_error("[TrashInfo] was found after first line");
                            }
                        }
                        if line.starts_with("Path=") && option_path.is_none() {
                            if !found_trash_info_line {
                                return trashinfo_error("Found Path line before TrashInfo");
                            }
                            let path_part = &line[5..];
                            info!("from_trash_info_file: encoded url {}", path_part);
                            let cow_path_str = url_escape::decode(path_part);
                            info!("from_trash_info_file: decoded url {}", cow_path_str);
                            let path_str = cow_path_str.as_ref();
                            option_path = Some(PathBuf::from(path_str));
                        } else if line.starts_with("DeletionDate=") && option_deleted_time.is_none()
                        {
                            if !found_trash_info_line {
                                return trashinfo_error("Found DeletionDate line before TrashInfo");
                            }
                            let deletion_date_str = &line[13..];
                            match parsed_date_from_path_info(deletion_date_str) {
                                Ok(()) => (),
                                Err(e) => return Err(e),
                            }
                            option_deleted_time = Some(deletion_date_str.to_owned())
                        }
                    }
                }
            }
            match (option_path, option_deleted_time) {
                (Some(origin), Some(deletion_date)) => {
                    info!("from_trash_info_file: {:?} parsed dest_name {} - deletion_date {} - origin {:?}", trash_info_file, dest_name, deletion_date, origin);
                    Ok(Self {
                        dest_name,
                        deletion_date,
                        origin,
                    })
                }
                _ => trashinfo_error("Couldn't parse the trash info file"),
            }
        } else {
            trashinfo_error("Couldn't parse the trash info filename")
        }
    }

    fn remove_extension(mut destname: String) -> FmResult<String> {
        if destname.ends_with(".trashinfo") {
            destname.truncate(destname.len() - 10);
            Ok(destname)
        } else {
            Err(FmError::custom(
                "trahsinfo",
                "filename doesn't contain .trashfino",
            ))
        }
    }
}

impl std::fmt::Display for TrashInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} - trashed on {}",
            path_to_string(&self.origin).unwrap_or_default(),
            self.deletion_date
        )
    }
}

/// Represent a view of the trash.
/// It's content is navigable so we use a Vector to hold the content.
#[derive(Clone)]
pub struct Trash {
    /// Trashed files info.
    pub content: Vec<TrashInfo>,
    index: usize,
    /// The path to the trashed files
    pub trash_folder_files: String,
    trash_folder_info: String,
}

impl Trash {
    fn pick_dest_name(&self, origin: &Path) -> FmResult<String> {
        if let Some(file_name) = origin.file_name() {
            let mut dest = file_name
                .to_str()
                .ok_or_else(|| {
                    FmError::custom(
                        "pick_dest_name",
                        "Couldn't parse the origin filename into a string",
                    )
                })?
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
        Err(FmError::custom(
            "pick_dest_name",
            "Couldn't extract the filename",
        ))
    }

    /// Parse the info files into a new instance.
    /// Only the file we can parse are read.
    pub fn parse_info_trashs() -> FmResult<Self> {
        let trash_folder_files = shellexpand::tilde(TRASH_FOLDER_FILES).to_string();
        let trash_folder_info = shellexpand::tilde(TRASH_FOLDER_INFO).to_string();
        let index = 0;

        match read_dir(&trash_folder_info) {
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

                Ok(Self {
                    content,
                    index,
                    trash_folder_files,
                    trash_folder_info,
                })
            }
            Err(error) => {
                info!("Couldn't read path {} - {}", trash_folder_files, error);
                Err(FmError::from(error))
            }
        }
    }

    /// Move a file to the trash folder and create a new trash info file.
    /// Add a new TrashInfo to the content.
    pub fn trash(&mut self, origin: PathBuf) -> FmResult<()> {
        if origin.is_relative() {
            return Err(FmError::custom("trash", "origin path shoudl be absolute"));
        }

        let dest_file_name = self.pick_dest_name(&origin)?;
        let trash_info = TrashInfo::new(&origin, &dest_file_name);
        let mut trashfile_filename = PathBuf::from(&self.trash_folder_files);
        trashfile_filename.push(&dest_file_name);

        let mut dest_trashinfo_name = dest_file_name.clone();
        dest_trashinfo_name.push_str(".trashinfo");
        let mut trashinfo_filename = PathBuf::from(&self.trash_folder_info);
        trashinfo_filename.push(dest_trashinfo_name);
        info!("moving to trash ... {:?} -> {:?}", origin, dest_file_name);
        trash_info.write_trash_info(&trashinfo_filename)?;

        self.content.push(trash_info);

        std::fs::rename(&origin, &trashfile_filename)?;

        info!("moved to trash {:?} -> {:?}", origin, dest_file_name);
        Ok(())
    }

    /// Empty the trash, removing all the files and the trashinfo.
    /// This action requires a confirmation.
    /// Watchout, it may delete files that weren't parsed.
    pub fn empty_trash(&mut self) -> FmResult<()> {
        remove_dir_all(&self.trash_folder_files)?;
        create_dir(&self.trash_folder_files)?;

        remove_dir_all(&self.trash_folder_info)?;
        create_dir(&self.trash_folder_info)?;
        let number_of_elements = self.content.len();

        self.content = vec![];

        info!("Emptied the trash: {} elements removed", number_of_elements);

        Ok(())
    }

    fn remove_selected_file(&mut self) -> FmResult<(PathBuf, PathBuf, PathBuf)> {
        let trashinfo = self.content[self.index].to_owned();

        let origin = trashinfo.origin;
        let dest_name = trashinfo.dest_name;
        let parent = find_parent(&origin)?;

        let mut info_name = dest_name.clone();
        info_name.push_str(".trashinfo");

        let mut trashed_file_content = PathBuf::from(&self.trash_folder_files);
        trashed_file_content.push(&dest_name);

        let mut trashed_file_info = PathBuf::from(&self.trash_folder_info);
        trashed_file_info.push(&info_name);

        if !trashed_file_content.exists() {
            return Err(FmError::custom(
                "trash restore",
                "Couldn't find the trashed file",
            ));
        }

        if !trashed_file_info.exists() {
            return Err(FmError::custom(
                "trash restore",
                "Couldn't find the trashed file info",
            ));
        }

        self.content.remove(self.index);
        std::fs::remove_file(&trashed_file_info)?;

        Ok((origin, trashed_file_content, parent))
    }

    /// Restores a file from the trash to its previous directory.
    /// If the parent (or ancestor) folder were deleted, it is recreated.
    pub fn restore(&mut self) -> FmResult<()> {
        let (origin, trashed_file_content, parent) = self.remove_selected_file()?;
        if !parent.exists() {
            std::fs::create_dir_all(&parent)?
        }
        match std::fs::rename(&trashed_file_content, &origin) {
            Ok(()) => info!(
                "trash restore: restored {:?} <- {:?}",
                origin, trashed_file_content
            ),
            Err(e) => info!("trash restore: rename error {:?}", e),
        }
        Ok(())
    }

    /// Deletes a file permanently from the trash.
    pub fn remove(&mut self) -> FmResult<()> {
        if self.is_empty() {
            return Ok(());
        }

        let (_, trashed_file_content, _) = self.remove_selected_file()?;

        std::fs::remove_file(&trashed_file_content)?;
        if self.index > 0 {
            self.index -= 1
        }
        Ok(())
    }
}

impl_selectable_content!(TrashInfo, Trash);

fn path_to_string(path: &Path) -> FmResult<&str> {
    path.to_str()
        .ok_or_else(|| FmError::custom("path_to_string", "couldn't parse origin into string"))
}

fn parsed_date_from_path_info(ds: &str) -> FmResult<()> {
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

fn trashinfo_error(msg: &str) -> FmResult<TrashInfo> {
    Err(FmError::custom("trash", msg))
}

fn find_parent(path: &Path) -> FmResult<PathBuf> {
    Ok(path
        .parent()
        .ok_or_else(|| {
            FmError::custom(
                "find_parent_as_string",
                &format!("Couldn't find parent of {:?}", path),
            )
        })?
        .to_owned())
}
