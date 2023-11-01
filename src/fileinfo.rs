use std::fs::{read_dir, symlink_metadata, DirEntry, Metadata};
use std::iter::Enumerate;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path;

use anyhow::{Context, Result};
use chrono::offset::Local;
use chrono::DateTime;
use log::info;
use tuikit::prelude::{Attr, Color, Effect};

use crate::colors::extension_color;
use crate::constant_strings_paths::PERMISSIONS_STR;
use crate::filter::FilterKind;
use crate::git::git;
use crate::impl_selectable_content;
use crate::sort::SortKind;
use crate::users::Users;
use crate::utils::filename_from_path;

type Valid = bool;

/// Different kind of files
#[derive(Debug, Clone, Copy)]
pub enum FileKind<Valid> {
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
    SymbolicLink(Valid),
}

impl FileKind<Valid> {
    /// Returns a new `FileKind` depending on metadata.
    /// Only linux files have some of those metadata
    /// since we rely on `std::fs::MetadataExt`.
    pub fn new(meta: &Metadata, filepath: &path::Path) -> Self {
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
            let valid = is_valid_symlink(filepath);
            Self::SymbolicLink(valid)
        } else {
            Self::NormalFile
        }
    }
    /// Returns the expected first symbol from `ln -l` line.
    /// d for directory, s for socket, . for file, c for char device,
    /// b for block, l for links.
    fn dir_symbol(&self) -> char {
        match self {
            FileKind::Fifo => 'p',
            FileKind::Socket => 's',
            FileKind::Directory => 'd',
            FileKind::NormalFile => '.',
            FileKind::CharDevice => 'c',
            FileKind::BlockDevice => 'b',
            FileKind::SymbolicLink(_) => 'l',
        }
    }

    fn sortable_char(&self) -> char {
        match self {
            FileKind::Directory => 'a',
            FileKind::NormalFile => 'b',
            FileKind::SymbolicLink(_) => 'c',
            FileKind::BlockDevice => 'd',
            FileKind::CharDevice => 'e',
            FileKind::Socket => 'f',
            FileKind::Fifo => 'g',
        }
    }
}

/// Different kind of display for the size column.
/// ls -lh display a human readable size for normal files,
/// nothing should be displayed for a directory,
/// Major & Minor driver versions are used for CharDevice & BlockDevice
#[derive(Clone, Debug)]
pub enum SizeColumn {
    /// Used for normal files. It's the size in bytes.
    Size(u64),
    /// Used for directories, nothing is displayed
    None,
    /// Use for CharDevice and BlockDevice.
    /// It's the major & minor driver versions.
    MajorMinor((u8, u8)),
}

impl std::fmt::Display for SizeColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Size(bytes) => write!(f, "   {hs}", hs = human_size(*bytes)),
            Self::None => write!(f, "     - "),
            Self::MajorMinor((major, minor)) => write!(f, "{major:>3},{minor:<3}"),
        }
    }
}

impl SizeColumn {
    fn new(size: u64, metadata: &Metadata, file_kind: &FileKind<Valid>) -> Self {
        match file_kind {
            FileKind::Directory => Self::None,
            FileKind::CharDevice | FileKind::BlockDevice => Self::MajorMinor(major_minor(metadata)),
            _ => Self::Size(size),
        }
    }
}

/// Infos about a file
/// We read and keep tracks every displayable information about
/// a file.
#[derive(Clone, Debug)]
pub struct FileInfo {
    /// Full path of the file
    pub path: path::PathBuf,
    /// Filename
    pub filename: String,
    /// File size as a `String`, already human formated.
    /// For char devices and block devices we display major & minor like ls.
    pub size_column: SizeColumn,
    /// True size of a file, not formated
    pub true_size: u64,
    /// Owner name of the file.
    pub owner: String,
    /// Group name of the file.
    pub group: String,
    /// System time of last modification
    pub system_time: String,
    /// Is this file currently selected ?
    pub is_selected: bool,
    /// What kind of file is this ?
    pub file_kind: FileKind<Valid>,
    /// Extension of the file. `""` for a directory.
    pub extension: String,
    /// A formated filename where the "kind" of file
    /// (directory, char device, block devive, fifo, socket, normal)
    /// is prepend to the name, allowing a "sort by kind" method.
    pub kind_format: String,
}

impl FileInfo {
    fn new(path: &path::Path, users: &Users) -> Result<Self> {
        let filename = extract_filename(path)?;
        let metadata = symlink_metadata(path)?;
        let path = path.to_owned();
        let owner = extract_owner(&metadata, users)?;
        let group = extract_group(&metadata, users)?;
        let system_time = extract_datetime(&metadata)?;
        let is_selected = false;
        let true_size = extract_file_size(&metadata);
        let file_kind = FileKind::new(&metadata, &path);
        let size_column = SizeColumn::new(true_size, &metadata, &file_kind);
        let extension = extract_extension(&path).into();
        let kind_format = filekind_and_filename(&filename, &file_kind);

        Ok(FileInfo {
            path,
            filename,
            size_column,
            true_size,
            owner,
            group,
            system_time,
            is_selected,
            file_kind,
            extension,
            kind_format,
        })
    }

    /// Reads every information about a file from its metadata and returs
    /// a new `FileInfo` object if we can create one.
    pub fn from_direntry(direntry: &DirEntry, users: &Users) -> Result<FileInfo> {
        Self::new(&direntry.path(), users)
    }

    /// Creates a fileinfo from a path and a filename.
    /// The filename is used when we create the fileinfo for "." and ".." in every folder.
    pub fn from_path_with_name(path: &path::Path, filename: &str, users: &Users) -> Result<Self> {
        let mut file_info = Self::new(path, users)?;
        file_info.filename = filename.to_owned();
        file_info.kind_format = filekind_and_filename(filename, &file_info.file_kind);
        Ok(file_info)
    }

    fn metadata(&self) -> Result<std::fs::Metadata> {
        Ok(symlink_metadata(&self.path)?)
    }

    /// String representation of file permissions
    pub fn permissions(&self) -> Result<String> {
        Ok(extract_permissions_string(&self.metadata()?))
    }

    /// Format the file line.
    /// Since files can have different owners in the same directory, we need to
    /// know the maximum size of owner column for formatting purpose.
    pub fn format(&self, owner_col_width: usize, group_col_width: usize) -> Result<String> {
        let mut repr = self.format_base(owner_col_width, group_col_width)?;
        repr.push(' ');
        repr.push_str(&self.filename);
        if let FileKind::SymbolicLink(_) = self.file_kind {
            match read_symlink_dest(&self.path) {
                Some(dest) => repr.push_str(&format!(" -> {dest}")),
                None => repr.push_str("  broken link"),
            }
        }
        Ok(repr)
    }

    fn format_base(&self, owner_col_width: usize, group_col_width: usize) -> Result<String> {
        let owner = format!("{owner:.owner_col_width$}", owner = self.owner,);
        let group = format!("{group:.group_col_width$}", group = self.group,);
        let repr = format!(
            "{dir_symbol}{permissions} {file_size} {owner:<owner_col_width$} {group:<group_col_width$} {system_time}",
            dir_symbol = self.dir_symbol(),
            permissions = self.permissions()?,
            file_size = self.size_column,
            system_time = self.system_time,
        );
        Ok(repr)
    }

    /// Format the metadata line, without the filename.
    /// Owned & Group have fixed width of 6.
    pub fn format_no_filename(&self) -> Result<String> {
        self.format_base(6, 6)
    }

    pub fn dir_symbol(&self) -> char {
        self.file_kind.dir_symbol()
    }

    pub fn format_simple(&self) -> Result<String> {
        Ok(self.filename.to_owned())
    }

    /// Select the file.
    pub fn select(&mut self) {
        self.is_selected = true;
    }

    /// Unselect the file.
    pub fn unselect(&mut self) {
        self.is_selected = false;
    }

    /// True iff the file is hidden (aka starts with a '.').
    pub fn is_hidden(&self) -> bool {
        self.filename.starts_with('.')
    }

    pub fn is_dir(&self) -> bool {
        self.path.is_dir()
    }

    /// Formated proper name.
    /// "/ " for `.`
    pub fn filename_without_dot_dotdot(&self) -> String {
        match self.filename.as_ref() {
            "." => "/ ".to_owned(),
            ".." => format!(
                "/{name} ",
                name = extract_filename(&self.path).unwrap_or_default()
            ),
            _ => format!("/{name} ", name = self.filename),
        }
    }
}

/// Holds the information about file in the current directory.
/// We know about the current path, the files themselves, the selected index,
/// the "display all files including hidden" flag and the key to sort files.
pub struct PathContent {
    /// The current path
    pub path: path::PathBuf,
    /// A vector of FileInfo with every file in current path
    pub content: Vec<FileInfo>,
    /// The index of the selected file.
    pub index: usize,
    /// The kind of sort used to display the files.
    pub sort_kind: SortKind,
    used_space: u64,
}

impl PathContent {
    /// Reads the paths and creates a new `PathContent`.
    /// Files are sorted by filename by default.
    /// Selects the first file if any.
    pub fn new(
        path: &path::Path,
        users: &Users,
        filter: &FilterKind,
        show_hidden: bool,
    ) -> Result<Self> {
        let path = path.to_owned();
        let mut content = Self::files(&path, show_hidden, filter, users)?;
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
        })
    }

    pub fn change_directory(
        &mut self,
        path: &path::Path,
        filter: &FilterKind,
        show_hidden: bool,
        users: &Users,
    ) -> Result<()> {
        self.content = Self::files(path, show_hidden, filter, users)?;
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
        users: &Users,
    ) -> Result<Vec<FileInfo>> {
        let mut files: Vec<FileInfo> = Self::create_dot_dotdot(path, users)?;

        let fileinfo = FileInfo::from_path_with_name(path, filename_from_path(path)?, users)?;
        if let Some(true_files) =
            files_collection(&fileinfo, users, show_hidden, filter_kind, false)
        {
            files.extend(true_files);
        }
        Ok(files)
    }

    fn create_dot_dotdot(path: &path::Path, users: &Users) -> Result<Vec<FileInfo>> {
        let current = FileInfo::from_path_with_name(path, ".", users)?;
        match path.parent() {
            Some(parent) => {
                let parent = FileInfo::from_path_with_name(parent, "..", users)?;
                Ok(vec![current, parent])
            }
            None => Ok(vec![current]),
        }
    }

    /// Convert a path to a &str.
    /// It may fails if the path contains non valid utf-8 chars.
    pub fn path_to_str(&self) -> String {
        self.path.display().to_string()
    }

    /// Sort the file with current key.
    pub fn sort(&mut self) {
        self.sort_kind.sort(&mut self.content)
    }

    /// Calculates the size of the owner column.
    pub fn owner_column_width(&self) -> usize {
        let owner_size_btreeset: std::collections::BTreeSet<usize> =
            self.content.iter().map(|file| file.owner.len()).collect();
        *owner_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Calculates the size of the group column.
    pub fn group_column_width(&self) -> usize {
        let group_size_btreeset: std::collections::BTreeSet<usize> =
            self.content.iter().map(|file| file.group.len()).collect();
        *group_size_btreeset.iter().next_back().unwrap_or(&1)
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
    pub fn reset_files(
        &mut self,
        filter: &FilterKind,
        show_hidden: bool,
        users: &Users,
    ) -> Result<()> {
        self.content = Self::files(&self.path, show_hidden, filter, users)?;
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
        Some(self.selected()?.path.display().to_string())
    }

    /// True if the path starts with a subpath.
    pub fn contains(&self, path: &path::Path) -> bool {
        path.starts_with(&self.path)
    }

    /// Is the selected file a directory ?
    /// It may fails if the current path is empty, aka if nothing is selected.
    pub fn is_selected_dir(&self) -> Result<bool> {
        match self
            .selected()
            .context("is selected dir: Empty directory")?
            .file_kind
        {
            FileKind::Directory => Ok(true),
            FileKind::SymbolicLink(true) => {
                let dest = read_symlink_dest(
                    &self
                        .selected()
                        .context("is selected dir: unreachable")?
                        .path,
                )
                .unwrap_or_default();
                Ok(path::PathBuf::from(dest).is_dir())
            }
            FileKind::SymbolicLink(false) => Ok(false),
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
    pub fn git_string(&self) -> Result<String> {
        git(&self.path)
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
    pub fn enumerate(&self) -> Enumerate<std::slice::Iter<'_, FileInfo>> {
        self.content.iter().enumerate()
    }

    /// Refresh the existing users.
    pub fn refresh_users(
        &mut self,
        users: &Users,
        filter: &FilterKind,
        show_hidden: bool,
    ) -> Result<()> {
        self.reset_files(filter, show_hidden, users)
    }

    /// Returns the correct index jump target to a flagged files.
    fn find_jump_index(&self, jump_target: &path::Path) -> Option<usize> {
        self.content
            .iter()
            .position(|file| file.path == jump_target)
    }

    /// Select the file from its path. Returns its index in content.
    pub fn select_file(&mut self, jump_target: &path::Path) -> usize {
        let index = self.find_jump_index(jump_target).unwrap_or_default();
        self.select_index(index);
        index
    }
}

impl_selectable_content!(FileInfo, PathContent);

fn fileinfo_color(fileinfo: &FileInfo) -> Color {
    match fileinfo.file_kind {
        FileKind::Directory => Color::RED,
        FileKind::BlockDevice => Color::YELLOW,
        FileKind::CharDevice => Color::GREEN,
        FileKind::Fifo => Color::BLUE,
        FileKind::Socket => Color::CYAN,
        FileKind::SymbolicLink(true) => Color::MAGENTA,
        FileKind::SymbolicLink(false) => Color::WHITE,
        _ => extension_color(&fileinfo.extension),
    }
}

/// Associates a filetype to `tuikit::prelude::Attr` : fg color, bg color and
/// effect.
/// Selected file is reversed.
pub fn fileinfo_attr(fileinfo: &FileInfo) -> Attr {
    let fg = fileinfo_color(fileinfo);

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
pub fn is_not_hidden(entry: &DirEntry) -> Result<bool> {
    let b = !entry
        .file_name()
        .to_str()
        .context("Couldn't read filename")?
        .starts_with('.');
    Ok(b)
}

fn extract_filename(path: &path::Path) -> Result<String> {
    Ok(path
        .file_name()
        .context("from path: couldn't read filename")?
        .to_str()
        .context("from path: couldn't parse filename")?
        .to_owned())
}

/// Returns the modified time.
fn extract_datetime(metadata: &Metadata) -> Result<String> {
    let datetime: DateTime<Local> = metadata.modified()?.into();
    Ok(format!("{}", datetime.format("%Y/%m/%d %T")))
}

/// Reads the permission and converts them into a string.
fn extract_permissions_string(metadata: &Metadata) -> String {
    let mode = (metadata.mode() & 511) as usize;
    let s_o = convert_octal_mode(mode >> 6);
    let s_g = convert_octal_mode((mode >> 3) & 7);
    let s_a = convert_octal_mode(mode & 7);
    format!("{s_o}{s_a}{s_g}")
}

/// Convert an integer like `Oo7` into its string representation like `"rwx"`
fn convert_octal_mode(mode: usize) -> &'static str {
    PERMISSIONS_STR[mode]
}

/// Reads the owner name and returns it as a string.
/// If it's not possible to get the owner name (happens if the owner exists on a remote machine but not on host),
/// it returns the uid as a  `Result<String>`.
fn extract_owner(metadata: &Metadata, users: &Users) -> Result<String> {
    match users.get_user_by_uid(metadata.uid()) {
        Some(name) => Ok(name),
        None => Ok(format!("{}", metadata.uid())),
    }
}

/// Reads the group name and returns it as a string.
/// If it's not possible to get the group name (happens if the group exists on a remote machine but not on host),
/// it returns the gid as a  `Result<String>`.
fn extract_group(metadata: &Metadata, users: &Users) -> Result<String> {
    match users.get_group_by_gid(metadata.gid()) {
        Some(name) => Ok(name),
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

/// Extract the major & minor driver version of a special file.
/// It's used for CharDevice & BlockDevice
fn major_minor(metadata: &Metadata) -> (u8, u8) {
    let device_ids = metadata.rdev().to_be_bytes();
    let major = device_ids[6];
    let minor = device_ids[7];
    (major, minor)
}

/// Extract the optional extension from a filename.
/// Returns empty &str aka "" if the file has no extension.
pub fn extract_extension(path: &path::Path) -> &str {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
}

fn get_used_space(files: &[FileInfo]) -> u64 {
    files.iter().map(|f| f.true_size).sum()
}

fn filekind_and_filename(filename: &str, file_kind: &FileKind<Valid>) -> String {
    format!("{c}{filename}", c = file_kind.sortable_char())
}

/// Creates an optional vector of fileinfo contained in a file.
/// Files are filtered by filterkind and the display hidden flag.
/// Returns None if there's no file.
pub fn files_collection(
    fileinfo: &FileInfo,
    users: &Users,
    show_hidden: bool,
    filter_kind: &FilterKind,
    keep_dir: bool,
) -> Option<Vec<FileInfo>> {
    match read_dir(&fileinfo.path) {
        Ok(read_dir) => Some(
            read_dir
                .filter_map(|direntry| direntry.ok())
                .filter(|direntry| show_hidden || is_not_hidden(direntry).unwrap_or(true))
                .map(|direntry| FileInfo::from_direntry(&direntry, users))
                .filter_map(|fileinfo| fileinfo.ok())
                .filter(|fileinfo| filter_kind.filter_by(fileinfo, keep_dir))
                .collect(),
        ),
        Err(error) => {
            info!(
                "Couldn't read path {path} - {error}",
                path = fileinfo.path.display(),
            );
            None
        }
    }
}

const MAX_PATH_ELEM_SIZE: usize = 50;

/// Shorten a path to be displayed in [`MAX_PATH_ELEM_SIZE`] chars or less.
/// Each element of the path is shortened if needed.
pub fn shorten_path(path: &path::Path, size: Option<usize>) -> Result<String> {
    let size = match size {
        Some(size) => size,
        None => MAX_PATH_ELEM_SIZE,
    };
    let path_string = path
        .to_str()
        .context("summarize: couldn't parse the path")?
        .to_owned();

    if path_string.len() < size {
        return Ok(path_string);
    }

    let splitted_path: Vec<_> = path_string.split('/').collect();
    let size_per_elem = std::cmp::max(1, size / (splitted_path.len() + 1)) + 1;
    let shortened_elems: Vec<_> = splitted_path
        .iter()
        .filter_map(|p| {
            if p.len() <= size_per_elem {
                Some(*p)
            } else {
                p.get(0..size_per_elem)
            }
        })
        .collect();
    Ok(shortened_elems.join("/"))
}

/// Returns `Some(destination)` where `destination` is a String if the path is
/// the destination of a symlink,
/// Returns `None` if the link is broken, if the path doesn't exists or if the path
/// isn't a symlink.
fn read_symlink_dest(path: &path::Path) -> Option<String> {
    match std::fs::read_link(path) {
        Ok(dest) if dest.exists() => Some(dest.to_str()?.to_owned()),
        _ => None,
    }
}

/// true iff the path is a valid symlink (pointing to an existing file).
fn is_valid_symlink(path: &path::Path) -> bool {
    matches!(std::fs::read_link(path), Ok(dest) if dest.exists())
}
