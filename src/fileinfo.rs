use std::fs::{metadata, read_dir, DirEntry, Metadata};
use std::iter::Enumerate;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path;
use std::rc::Rc;

use chrono::offset::Local;
use chrono::DateTime;
use log::info;
use tuikit::prelude::{Attr, Color, Effect};
use users::{Groups, Users, UsersCache};

use crate::config::{str_to_tuikit, Colors};
use crate::constant_strings_paths::PERMISSIONS_STR;
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::git::git;
use crate::impl_selectable_content;
use crate::sort::SortKind;
use crate::utils::filename_from_path;

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
    pub fn new(meta: &Metadata) -> Self {
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
    }
    /// Returns the expected first symbol from `ln -l` line.
    /// d for directory, s for socket, . for file, c for char device,
    /// b for block, l for links.
    fn extract_dir_symbol(&self) -> char {
        match self {
            FileKind::Fifo => 'p',
            FileKind::Socket => 's',
            FileKind::Directory => 'd',
            FileKind::NormalFile => '.',
            FileKind::CharDevice => 'c',
            FileKind::BlockDevice => 'b',
            FileKind::SymbolicLink => 'l',
        }
    }

    fn sortable_char(&self) -> char {
        match self {
            FileKind::Directory => 'a',
            FileKind::NormalFile => 'b',
            FileKind::SymbolicLink => 'c',
            FileKind::BlockDevice => 'd',
            FileKind::CharDevice => 'e',
            FileKind::Socket => 'f',
            FileKind::Fifo => 'g',
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
    pub dir_symbol: char,
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
    /// A formated filename where the "kind" of file
    /// (directory, char device, block devive, fifo, socket, normal)
    /// is prepend to the name, allowing a "sort by kind" method.
    pub kind_format: String,
}

impl FileInfo {
    /// Reads every information about a file from its metadata and returs
    /// a new `FileInfo` object if we can create one.
    pub fn new(direntry: &DirEntry, users_cache: &Rc<UsersCache>) -> FmResult<FileInfo> {
        let metadata = direntry.metadata()?;
        let path = direntry.path();
        let filename = extract_filename(direntry)?;

        Self::create_from_metadata_and_filename(&path, &metadata, filename, users_cache)
    }

    /// Creates a fileinfo from a path and a filename.
    /// The filename is used when we create the fileinfo for "." and ".." in every folder.
    pub fn from_path_with_name(
        path: &path::Path,
        filename: &str,
        users_cache: &Rc<UsersCache>,
    ) -> FmResult<Self> {
        let metadata = metadata(path)?;

        Self::create_from_metadata_and_filename(path, &metadata, filename.to_owned(), users_cache)
    }

    pub fn from_path(path: &path::Path, users_cache: &Rc<UsersCache>) -> FmResult<Self> {
        let metadata = metadata(path)?;
        Self::create_from_metadata_and_filename(
            path,
            &metadata,
            path.file_name()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_owned(),
            users_cache,
        )
    }

    fn create_from_metadata_and_filename(
        path: &path::Path,
        metadata: &Metadata,
        filename: String,
        users_cache: &Rc<UsersCache>,
    ) -> FmResult<Self> {
        let path = path.to_owned();
        let size = extract_file_size(metadata);
        let file_size = human_size(size);
        let permissions = extract_permissions_string(metadata)?;
        let owner = extract_owner(metadata, users_cache)?;
        let group = extract_group(metadata, users_cache)?;
        let system_time = extract_datetime(metadata)?;
        let is_selected = false;

        let file_kind = FileKind::new(metadata);
        let dir_symbol = file_kind.extract_dir_symbol();
        let extension = extract_extension(&path).into();
        let kind_format = filekind_and_filename(&filename, &file_kind);

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
            kind_format,
        })
    }
    /// Format the file line.
    /// Since files can have different owners in the same directory, we need to
    /// know the maximum size of owner column for formatting purpose.
    pub fn format(&self, owner_col_width: usize, group_col_width: usize) -> FmResult<String> {
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

    fn format_simple(&self) -> FmResult<String> {
        Ok(self.filename.to_owned())
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

    pub fn is_hidden(&self) -> bool {
        self.filename.starts_with('.')
    }
}

/// Holds the information about file in the current directory.
/// We know about the current path, the files themselves, the selected index,
/// the "display all files including hidden" flag and the key to sort files.
#[derive(Clone)]
pub struct PathContent {
    /// The current path
    pub path: path::PathBuf,
    /// A vector of FileInfo with every file in current path
    pub content: Vec<FileInfo>,
    /// The index of the selected file.
    pub index: usize,
    /// The kind of sort used to display the files.
    sort_kind: SortKind,
    used_space: u64,
    pub users_cache: Rc<UsersCache>,
}

impl PathContent {
    /// Reads the paths and creates a new `PathContent`.
    /// Files are sorted by filename by default.
    /// Selects the first file if any.
    pub fn new(
        path: &path::Path,
        users_cache: Rc<UsersCache>,
        filter: &FilterKind,
        show_hidden: bool,
    ) -> FmResult<Self> {
        let path = path.to_owned();
        let mut content = Self::files(&path, show_hidden, filter, &users_cache)?;
        let sort_kind = SortKind::default();
        sort_kind.sort(&mut content);
        let selected_index: usize = 0;
        if !content.is_empty() {
            content[selected_index].select();
        }
        let used_space = get_used_space(&content);

        Ok(Self {
            path,
            content,
            index: selected_index,
            sort_kind,
            used_space,
            users_cache,
        })
    }

    pub fn change_directory(
        &mut self,
        path: &path::Path,
        filter: &FilterKind,
        show_hidden: bool,
    ) -> FmResult<()> {
        self.content = Self::files(path, show_hidden, filter, &self.users_cache)?;
        self.sort_kind.sort(&mut self.content);
        self.index = 0;
        if !self.content.is_empty() {
            self.content[0].select()
        }
        self.used_space = get_used_space(&self.content);
        self.path = path.to_path_buf();
        Ok(())
    }

    fn files(
        path: &path::Path,
        show_hidden: bool,
        filter_kind: &FilterKind,
        users_cache: &Rc<UsersCache>,
    ) -> FmResult<Vec<FileInfo>> {
        let mut files: Vec<FileInfo> = Self::create_dot_dotdot(path, users_cache)?;

        let fileinfo = FileInfo::from_path_with_name(path, filename_from_path(path)?, users_cache)?;
        if let Some(true_files) = files_collection(&fileinfo, users_cache, show_hidden, filter_kind)
        {
            files.extend(true_files);
        }
        Ok(files)
    }

    fn create_dot_dotdot(
        path: &path::Path,
        users_cache: &Rc<UsersCache>,
    ) -> FmResult<Vec<FileInfo>> {
        let current = FileInfo::from_path_with_name(path, ".", users_cache)?;
        match path.parent() {
            Some(parent) => {
                let parent = FileInfo::from_path_with_name(parent, "..", users_cache)?;
                Ok(vec![current, parent])
            }
            None => Ok(vec![current]),
        }
    }

    /// Convert a path to a &str.
    /// It may fails if the path contains non valid utf-8 chars.
    pub fn path_to_str(&self) -> FmResult<&str> {
        self.path
            .to_str()
            .ok_or_else(|| FmError::custom("path to str", "Unreadable path"))
    }

    /// Sort the file with current key.
    pub fn sort(&mut self) {
        self.sort_kind.sort(&mut self.content)
    }

    /// Calculates the size of the owner column.
    fn owner_column_width(&self) -> usize {
        let owner_size_btreeset: std::collections::BTreeSet<usize> =
            self.content.iter().map(|file| file.owner.len()).collect();
        *owner_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Calculates the size of the group column.
    fn group_column_width(&self) -> usize {
        let group_size_btreeset: std::collections::BTreeSet<usize> =
            self.content.iter().map(|file| file.group.len()).collect();
        *group_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Returns a vector of displayable strings for every file.
    pub fn strings(&self, display_full: bool) -> Vec<String> {
        if display_full {
            let owner_size = self.owner_column_width();
            let group_size = self.group_column_width();
            self.content
                .iter()
                .map(|fileinfo| fileinfo.format(owner_size, group_size).unwrap_or_default())
                .collect()
        } else {
            self.content
                .iter()
                .map(|fileinfo| fileinfo.format_simple().unwrap_or_default())
                .collect()
        }
    }

    /// Select the file from a given index.
    pub fn select_index(&mut self, index: usize) {
        if index < self.content.len() {
            self.unselect_current();
            self.content[index].select();
            self.index = index;
        }
    }

    /// Reset the current file content.
    /// Reads and sort the content with current key.
    /// Select the first file if any.
    pub fn reset_files(&mut self, filter: &FilterKind, show_hidden: bool) -> Result<(), FmError> {
        self.content = Self::files(&self.path, show_hidden, filter, &self.users_cache)?;
        self.sort_kind = SortKind::default();
        self.sort();
        self.index = 0;
        if !self.content.is_empty() {
            self.content[self.index].select();
        }
        Ok(())
    }

    /// Path of the currently selected file.
    pub fn selected_path_string(&self) -> Option<String> {
        Some(self.selected()?.path.to_str()?.to_owned())
    }

    /// True if the path starts with a subpath.
    pub fn contains(&self, path: &path::Path) -> bool {
        path.starts_with(&self.path)
    }

    /// Is the selected file a directory ?
    /// It may fails if the current path is empty, aka if nothing is selected.
    pub fn is_selected_dir(&self) -> FmResult<bool> {
        match self
            .selected()
            .ok_or_else(|| FmError::custom("is selected dir", "Empty directory"))?
            .file_kind
        {
            FileKind::Directory => Ok(true),
            FileKind::SymbolicLink => {
                let dest = self
                    .selected()
                    .ok_or_else(|| FmError::custom("is selected dir", "unreachable"))?
                    .read_dest()
                    .unwrap_or_default();
                Ok(path::PathBuf::from(dest).is_dir())
            }
            _ => Ok(false),
        }
    }
    pub fn true_len(&self) -> usize {
        match self.path.parent() {
            Some(_) => self.content.len() - 2,
            None => self.content.len() - 1,
        }
    }

    /// Human readable string representation of the space used by _files_
    /// in current path.
    /// No recursive exploration of directory.
    pub fn used_space(&self) -> String {
        human_size(self.used_space)
    }

    /// A string representation of the git status of the path.
    pub fn git_string(&self) -> FmResult<String> {
        Ok(git(&self.path)?)
    }

    /// Update the kind of sort from a char typed by the user.
    pub fn update_sort_from_char(&mut self, c: char) {
        self.sort_kind.update_from_char(c)
    }

    /// Unselect the current item.
    /// Since we use a common trait to navigate the files,
    /// this method is required.
    pub fn unselect_current(&mut self) {
        if self.is_empty() {
            return;
        }
        self.content[self.index].unselect();
    }

    /// Select the current item.
    /// Since we use a common trait to navigate the files,
    /// this method is required.
    pub fn select_current(&mut self) {
        if self.is_empty() {
            return;
        }
        self.content[self.index].select();
    }

    /// Returns an enumeration of the files (`FileInfo`) in content.
    pub fn enumerate(&mut self) -> Enumerate<std::slice::Iter<'_, FileInfo>> {
        self.content.iter().enumerate()
    }

    /// Refresh the existing users.
    pub fn refresh_users(
        &mut self,
        users_cache: Rc<UsersCache>,
        filter: &FilterKind,
        show_hidden: bool,
    ) -> FmResult<()> {
        self.users_cache = users_cache;
        self.reset_files(filter, show_hidden)
    }
}

impl_selectable_content!(FileInfo, PathContent);

/// Associates a filetype to `tuikit::prelude::Attr` : fg color, bg color and
/// effect.
/// Selected file is reversed.
///
/// TODO! can be refactored to only use 2 parameters by using the config_color from status
pub fn fileinfo_attr(fileinfo: &FileInfo, colors: &Colors) -> Attr {
    let fg = match fileinfo.file_kind {
        FileKind::Directory => str_to_tuikit(&colors.directory),
        FileKind::BlockDevice => str_to_tuikit(&colors.block),
        FileKind::CharDevice => str_to_tuikit(&colors.char),
        FileKind::Fifo => str_to_tuikit(&colors.fifo),
        FileKind::Socket => str_to_tuikit(&colors.socket),
        FileKind::SymbolicLink => str_to_tuikit(&colors.symlink),
        _ => colors.color_cache.extension_color(&fileinfo.extension),
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

/// True if the file isn't hidden.
pub fn is_not_hidden(entry: &DirEntry) -> FmResult<bool> {
    Ok(entry
        .file_name()
        .into_string()
        .map(|s| !s.starts_with('.'))?)
}

/// Returns the modified time.
fn extract_datetime(metadata: &Metadata) -> FmResult<String> {
    let datetime: DateTime<Local> = metadata.modified()?.into();
    Ok(format!("{}", datetime.format("%d/%m/%Y %T")))
}

/// Returns the filename.
fn extract_filename(direntry: &DirEntry) -> FmResult<String> {
    Ok(direntry.file_name().into_string()?)
}

/// Reads the permission and converts them into a string.
fn extract_permissions_string(metadata: &Metadata) -> FmResult<String> {
    let mut perm = String::with_capacity(9);
    let mode = (metadata.mode() & 511) as usize;
    let s_o = convert_octal_mode(mode >> 6);
    let s_g = convert_octal_mode((mode >> 3) & 7);
    let s_a = convert_octal_mode(mode & 7);
    perm.push_str(s_o);
    perm.push_str(s_a);
    perm.push_str(s_g);
    Ok(perm)
}

/// Convert an integer like `Oo7` into its string representation like `"rwx"`
fn convert_octal_mode(mode: usize) -> &'static str {
    PERMISSIONS_STR[mode]
}

/// Reads the owner name and returns it as a string.
/// If it's not possible to get the owner name (happens if the owner exists on a remote machine but not on host),
/// it returns the uid as a  `Result<String>`.
fn extract_owner(metadata: &Metadata, users_cache: &Rc<UsersCache>) -> FmResult<String> {
    match users_cache.get_user_by_uid(metadata.uid()) {
        Some(uid) => Ok(uid
            .name()
            .to_str()
            .ok_or_else(|| FmError::custom("extract owner", "Couldn't parse owner name"))?
            .to_owned()),
        None => Ok(format!("{}", metadata.uid())),
    }
}

/// Reads the group name and returns it as a string.
/// If it's not possible to get the group name (happens if the group exists on a remote machine but not on host),
/// it returns the gid as a  `Result<String>`.
fn extract_group(metadata: &Metadata, users_cache: &Rc<UsersCache>) -> FmResult<String> {
    match users_cache.get_group_by_gid(metadata.gid()) {
        Some(gid) => Ok(gid
            .name()
            .to_str()
            .ok_or_else(|| FmError::custom("extract group", "Couldn't parse group name"))?
            .to_owned()),
        None => Ok(format!("{}", metadata.gid())),
    }
}

/// Returns the file size.
fn extract_file_size(metadata: &Metadata) -> u64 {
    metadata.len()
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
pub fn extract_extension(path: &path::Path) -> &str {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
}

fn get_used_space(files: &[FileInfo]) -> u64 {
    files.iter().map(|f| f.size).sum()
}

fn filekind_and_filename(filename: &str, file_kind: &FileKind) -> String {
    let mut s = String::new();
    s.push(file_kind.sortable_char());
    s.push_str(filename);
    s
}

/// Creates an optional vector of fileinfo contained in a file.
/// Files are filtered by filterkind and the display hidden flag.
/// Returns None if there's no file.
pub fn files_collection(
    fileinfo: &FileInfo,
    users_cache: &Rc<UsersCache>,
    show_hidden: bool,
    filter_kind: &FilterKind,
) -> Option<Vec<FileInfo>> {
    match read_dir(&fileinfo.path) {
        Ok(read_dir) => Some(
            read_dir
                .filter_map(|direntry| direntry.ok())
                .filter(|direntry| show_hidden || is_not_hidden(direntry).unwrap_or(true))
                .map(|direntry| FileInfo::new(&direntry, users_cache))
                .filter_map(|fileinfo| fileinfo.ok())
                .filter(|fileinfo| filter_kind.filter_by(fileinfo))
                .collect(),
        ),
        Err(error) => {
            info!(
                "Couldn't read path {} - {}",
                fileinfo.path.to_string_lossy(),
                error
            );
            None
        }
    }
}
