use std::fs::{metadata, read_dir, DirEntry};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path;

use chrono::offset::Local;
use chrono::DateTime;
use log::info;
use tuikit::prelude::{Attr, Color, Effect};
use users::{get_group_by_gid, get_user_by_uid};

use crate::config::{str_to_tuikit, Colors};
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::git::git;
use crate::status::Status;

/// Different kind of sort
#[derive(Debug, Clone)]
pub enum SortBy {
    /// Directory first
    Kind,
    /// by filename
    Filename,
    /// by date
    Date,
    /// by size
    Size,
    /// by extension
    Extension,
}

impl SortBy {
    /// Returns a key for each variant.
    /// The key can be used to sort the directory content.
    pub fn key(&self, file: &FileInfo) -> String {
        match *self {
            Self::Kind => Self::sort_by_kind(file),
            Self::Filename => file.filename.clone(),
            Self::Date => file.system_time.clone(),
            Self::Size => file.file_size.clone(),
            Self::Extension => file.extension.clone(),
        }
    }

    fn sort_by_kind(file: &FileInfo) -> String {
        let mut s = String::new();
        match file.file_kind {
            FileKind::Directory => s.push('a'),
            FileKind::NormalFile => s.push('b'),
            FileKind::SymbolicLink => s.push('c'),
            FileKind::BlockDevice => s.push('d'),
            FileKind::CharDevice => s.push('e'),
            FileKind::Socket => s.push('f'),
            FileKind::Fifo => s.push('g'),
        }
        s.push_str(&file.filename);
        s
    }
}

/// Different kind of files
#[derive(Debug, Clone, Copy)]
pub enum FileKind {
    /// Classic files.
    NormalFile,
    /// Folder
    Directory,
    /// Block devices like /sda1
    BlockDevice,
    /// Char devices like /dev/null
    CharDevice,
    /// Named pipes
    Fifo,
    /// File socket
    Socket,
    /// symlink
    SymbolicLink,
}

impl FileKind {
    /// Returns a new `FileKind` depending on metadata.
    /// Only linux files have some of those metadata
    /// since we rely on `std::fs::MetadataExt`.
    pub fn new(direntry: &DirEntry) -> Self {
        if let Ok(meta) = direntry.metadata() {
            if meta.file_type().is_dir() {
                Self::Directory
            } else if meta.file_type().is_block_device() {
                Self::BlockDevice
            } else if meta.file_type().is_socket() {
                Self::Socket
            } else if meta.file_type().is_char_device() {
                Self::CharDevice
            } else if meta.file_type().is_fifo() {
                Self::Fifo
            } else if meta.file_type().is_symlink() {
                Self::SymbolicLink
            } else {
                Self::NormalFile
            }
        } else {
            Self::NormalFile
        }
    }
    /// Returns the expected first symbol from `ln -l` line.
    /// d for directory, s for socket, . for file, c for char device,
    /// b for block, l for links.
    fn extract_dir_symbol(&self) -> String {
        match self {
            FileKind::Fifo => "p".to_owned(),
            FileKind::Socket => "s".to_owned(),
            FileKind::Directory => "d".to_owned(),
            FileKind::NormalFile => ".".to_owned(),
            FileKind::CharDevice => "c".to_owned(),
            FileKind::BlockDevice => "b".to_owned(),
            FileKind::SymbolicLink => "l".to_owned(),
        }
    }
}

/// Infos about a file
/// We read and keep tracks every displayable information about
/// a file.
/// Like in [exa](https://github.com/ogham/exa) we don't display the group.
#[derive(Clone, Debug)]
pub struct FileInfo {
    /// Full path of the file
    pub path: path::PathBuf,
    /// Filename
    pub filename: String,
    /// size (nb of bytes) of the file
    pub size: u64,
    /// File size as a `String`, already human formated.
    pub file_size: String,
    /// First symbol displaying the kind of file.
    pub dir_symbol: String,
    /// Str formatted permissions like rwxr..rw.
    pub permissions: String,
    /// Owner name of the file.
    pub owner: String,
    /// Group name of the file.
    pub group: String,
    /// System time of last modification
    pub system_time: String,
    /// Is this file currently selected ?
    pub is_selected: bool,
    /// What kind of file is this ?
    pub file_kind: FileKind,
    /// Extension of the file. `""` for a directory.
    pub extension: String,
}

impl FileInfo {
    /// Reads every information about a file from its metadata and returs
    /// a new `FileInfo` object if we can create one.
    pub fn new(direntry: &DirEntry) -> FmResult<FileInfo> {
        let path = direntry.path();
        let filename = extract_filename(direntry)?;
        let size = extract_file_size(direntry)?;
        let file_size = human_size(size);
        let permissions = extract_permissions_string(direntry)?;
        let owner = extract_owner(direntry)?;
        let group = extract_group(direntry)?;
        let system_time = extract_datetime(direntry)?;
        let is_selected = false;

        let file_kind = FileKind::new(direntry);
        let dir_symbol = file_kind.extract_dir_symbol();
        let extension = extract_extension_from_filename(&filename).into();

        Ok(FileInfo {
            path,
            filename,
            size,
            file_size,
            dir_symbol,
            permissions,
            owner,
            group,
            system_time,
            is_selected,
            file_kind,
            extension,
        })
    }

    /// Format the file line.
    /// Since files can have different owners in the same directory, we need to
    /// know the maximum size of owner column for formatting purpose.
    fn format(&self, owner_col_width: usize, group_col_width: usize) -> FmResult<String> {
        let mut repr = format!(
            "{dir_symbol}{permissions} {file_size} {owner:<owner_col_width$} {group:<group_col_width$} {system_time} {filename}",
            dir_symbol = self.dir_symbol,
            permissions = self.permissions,
            file_size = self.file_size,
            owner = self.owner,
            owner_col_width = owner_col_width,
            group = self.group,
            group_col_width = group_col_width,
            system_time = self.system_time,
            filename = self.filename,
        );
        if let FileKind::SymbolicLink = self.file_kind {
            repr.push_str(" -> ");
            repr.push_str(&self.read_dest().unwrap_or_else(|| "Broken link".to_owned()));
        }
        Ok(repr)
    }

    fn read_dest(&self) -> Option<String> {
        match metadata(&self.path) {
            Ok(_) => Some(std::fs::read_link(&self.path).ok()?.to_str()?.to_owned()),
            Err(_) => Some("Broken link".to_owned()),
        }
    }

    /// Select the file.
    pub fn select(&mut self) {
        self.is_selected = true;
    }

    /// Unselect the file.
    pub fn unselect(&mut self) {
        self.is_selected = false;
    }
}

/// Holds the information about file in the current directory.
/// We know about the current path, the files themselves, the selected index,
/// the "display all files including hidden" flag and the key to sort files.
#[derive(Clone)]
pub struct PathContent {
    pub path: path::PathBuf,
    pub files: Vec<FileInfo>,
    pub selected: usize,
    pub show_hidden: bool,
    pub sort_by: SortBy,
    pub reverse: bool,
    filter: FilterKind,
    used_space: u64,
}

impl PathContent {
    /// Reads the paths and creates a new `PathContent`.
    /// Files are sorted by filename by default.
    /// Selects the first file if any.
    pub fn new(path: path::PathBuf, show_hidden: bool) -> Result<Self, FmError> {
        let filter = FilterKind::All;
        let mut files = Self::files(&path, show_hidden, filter.clone())?;
        let sort_by = SortBy::Kind;
        files.sort_by_key(|file| sort_by.key(file));
        let selected: usize = 0;
        if !files.is_empty() {
            files[selected].select();
        }
        let reverse = false;
        let used_space = get_size(path.clone()).unwrap_or_default();

        Ok(Self {
            path,
            files,
            selected,
            show_hidden,
            sort_by,
            reverse,
            filter,
            used_space,
        })
    }

    pub fn set_filter(&mut self, filter: FilterKind) {
        self.filter = filter
    }

    fn files(path: &path::Path, show_hidden: bool, filter: FilterKind) -> FmResult<Vec<FileInfo>> {
        match read_dir(path) {
            Ok(read_dir) => {
                let files: Vec<FileInfo> = if show_hidden {
                    read_dir
                        .filter_map(|res_direntry| res_direntry.ok())
                        .map(|direntry| FileInfo::new(&direntry))
                        .filter_map(|res_file_entry| res_file_entry.ok())
                        .filter(|fileinfo| filter.filter_by(fileinfo))
                        .collect()
                } else {
                    read_dir
                        .filter_map(|res_direntry| res_direntry.ok())
                        .filter(|e| is_not_hidden(e).unwrap_or(true))
                        .map(|direntry| FileInfo::new(&direntry))
                        .filter_map(|res_file_entry| res_file_entry.ok())
                        .filter(|fileinfo| filter.filter_by(fileinfo))
                        .collect()
                };
                Ok(files)
            }
            Err(error) => {
                info!("Couldn't read path {} - {}", path.to_string_lossy(), error);
                Ok(vec![])
            }
        }
    }

    pub fn path_to_str(&self) -> FmResult<&str> {
        self.path
            .to_str()
            .ok_or_else(|| FmError::new("Unreadable path"))
    }

    /// Sort the file with current key.
    pub fn sort(&mut self) {
        self.files.sort_by_key(|file| self.sort_by.key(file));
    }

    /// Calculates the size of the owner column.
    fn owner_column_width(&self) -> usize {
        let mut owner_size_btreeset = std::collections::BTreeSet::new();
        for file in self.files.iter() {
            owner_size_btreeset.insert(file.owner.len());
        }
        *owner_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Calculates the size of the group column.
    fn group_column_width(&self) -> usize {
        let mut group_size_btreeset = std::collections::BTreeSet::new();
        for file in self.files.iter() {
            group_size_btreeset.insert(file.group.len());
        }
        *group_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Returns a vector of displayable strings for every file.
    pub fn strings(&self) -> Vec<String> {
        let owner_size = self.owner_column_width();
        let group_size = self.group_column_width();
        self.files
            .iter()
            .map(|fileinfo| fileinfo.format(owner_size, group_size).unwrap_or_default())
            .collect()
    }

    /// Select the next file, if any.
    pub fn select_next(&mut self) {
        if !self.files.is_empty() && self.selected < self.files.len() - 1 {
            self.files[self.selected].unselect();
            self.selected += 1;
            self.files[self.selected].select();
        }
    }

    /// Select the previous file, if any.
    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.files[self.selected].unselect();
            self.selected -= 1;
            self.files[self.selected].select();
        }
    }

    /// Select the file from a given index.
    pub fn select_index(&mut self, index: usize) {
        if index < self.files.len() {
            self.files[self.selected].unselect();
            self.files[index].select();
            self.selected = index;
        }
    }

    /// Reset the current file content.
    /// Reads and sort the content with current key.
    /// Select the first file if any.
    pub fn reset_files(&mut self) -> Result<(), FmError> {
        self.files = Self::files(&self.path, self.show_hidden, self.filter.clone())?;
        self.sort_by = SortBy::Kind;
        self.sort();
        self.selected = 0;
        if !self.files.is_empty() {
            self.files[self.selected].select();
        }
        Ok(())
    }

    /// Return the Optional FileInfo
    /// Since the FileInfo is borrowed it won't be mutable.
    pub fn selected_file(&self) -> Option<&FileInfo> {
        if self.files.is_empty() {
            None
        } else {
            Some(&self.files[self.selected])
        }
    }

    pub fn selected_path_str(&self) -> Option<String> {
        Some(self.selected_file()?.path.to_str()?.to_owned())
    }

    pub fn contains(&self, path: &path::Path) -> bool {
        path.starts_with(&self.path)
    }

    pub fn is_selected_dir(&self) -> FmResult<bool> {
        match self
            .selected_file()
            .ok_or_else(|| FmError::new("Empty directory"))?
            .file_kind
        {
            FileKind::Directory => Ok(true),
            FileKind::SymbolicLink => {
                let dest = self
                    .selected_file()
                    .ok_or_else(|| FmError::new("unreachable"))?
                    .read_dest()
                    .unwrap_or_default();
                Ok(path::PathBuf::from(dest).is_dir())
            }
            _ => Ok(false),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn used_space(&self) -> String {
        human_size(self.used_space)
    }

    pub fn git_string(&self) -> FmResult<String> {
        Ok(git(&self.path)?)
    }
}

/// Associates a filetype to `tuikit::prelude::Attr` : fg color, bg color and
/// effect.
/// Selected file is reversed.
pub fn fileinfo_attr(status: &Status, fileinfo: &FileInfo, colors: &Colors) -> Attr {
    let fg = match fileinfo.file_kind {
        FileKind::Directory => str_to_tuikit(&colors.directory),
        FileKind::BlockDevice => str_to_tuikit(&colors.block),
        FileKind::CharDevice => str_to_tuikit(&colors.char),
        FileKind::Fifo => str_to_tuikit(&colors.fifo),
        FileKind::Socket => str_to_tuikit(&colors.socket),
        FileKind::SymbolicLink => str_to_tuikit(&colors.symlink),
        _ => status.colors.extension_color(&fileinfo.extension),
    };

    let effect = if fileinfo.is_selected {
        Effect::REVERSE
    } else {
        Effect::empty()
    };

    Attr {
        fg,
        bg: Color::default(),
        effect,
    }
}

/// true if the file isn't hidden.
fn is_not_hidden(entry: &DirEntry) -> Result<bool, FmError> {
    Ok(entry
        .file_name()
        .into_string()
        .map(|s| !s.starts_with('.'))?)
}

/// Returns the modified time.
fn extract_datetime(direntry: &DirEntry) -> FmResult<String> {
    let datetime: DateTime<Local> = direntry.metadata()?.modified()?.into();
    Ok(format!("{}", datetime.format("%d/%m/%Y %T")))
}

/// Returns the filename.
fn extract_filename(direntry: &DirEntry) -> FmResult<String> {
    Ok(direntry.file_name().into_string()?)
}

/// Reads the permission and converts them into a string.
fn extract_permissions_string(direntry: &DirEntry) -> FmResult<String> {
    match metadata(direntry.path()) {
        Ok(metadata) => {
            let mode = metadata.mode() & 511;
            let s_o = convert_octal_mode(mode >> 6);
            let s_g = convert_octal_mode((mode >> 3) & 7);
            let s_a = convert_octal_mode(mode & 7);
            Ok([s_o, s_g, s_a].join(""))
        }
        Err(_) => Ok("??????".to_owned()),
    }
}

/// Convert an integer like `Oo777` into its string representation like `"rwx"`
fn convert_octal_mode(mode: u32) -> String {
    let rwx = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
    String::from(rwx[(mode & 7_u32) as usize])
}

/// Reads the owner name and returns it as a string.
fn extract_owner(direntry: &DirEntry) -> FmResult<String> {
    match metadata(direntry.path()) {
        Ok(metadata) => Ok(String::from(
            get_user_by_uid(metadata.uid())
                .ok_or_else(|| FmError::new("Couldn't read uid"))?
                .name()
                .to_str()
                .ok_or_else(|| FmError::new("Couldn't read owner name"))?,
        )),
        Err(_) => Ok("".to_owned()),
    }
}

/// Reads the group name and returns it as a string.
fn extract_group(direntry: &DirEntry) -> FmResult<String> {
    match metadata(direntry.path()) {
        Ok(metadata) => Ok(String::from(
            get_group_by_gid(metadata.gid())
                .ok_or_else(|| FmError::new("Couldn't read gid"))?
                .name()
                .to_str()
                .ok_or_else(|| FmError::new("Couldn't read group name"))?,
        )),
        Err(_) => Ok("".to_owned()),
    }
}

/// Returns the file size.
fn extract_file_size(direntry: &DirEntry) -> Result<u64, FmError> {
    match direntry.path().metadata() {
        Ok(metadata) => Ok(metadata.len()),
        Err(_) => Ok(0),
    }
}

/// Convert a file size from bytes to human readable string.
pub fn human_size(bytes: u64) -> String {
    let size = ["", "k", "M", "G", "T", "P", "E", "Z", "Y"];
    let factor = (bytes.to_string().chars().count() as u64 - 1) / 3_u64;
    format!(
        "{:>3}{:<1}",
        bytes / (1024_u64).pow(factor as u32),
        size[factor as usize]
    )
}

/// Extract the optional extension from a filename.
/// Returns empty &str aka "" if the file has no extension.
fn extract_extension_from_filename(filename: &str) -> &str {
    path::Path::new(filename)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
}

fn get_size(path: path::PathBuf) -> FmResult<u64> {
    let mut result = 0;

    if path.is_dir() {
        for entry in read_dir(&path)? {
            let _path = entry?.path();
            if _path.is_file() {
                result += _path.metadata()?.len();
            }
        }
    } else {
        result = path.metadata()?.len();
    }
    Ok(result)
}
