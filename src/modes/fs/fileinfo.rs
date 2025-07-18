use std::fs::{symlink_metadata, DirEntry, Metadata};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::offset::Local;
use chrono::DateTime;
use ratatui::style::Style;

use crate::config::{extension_color, FILE_STYLES};
use crate::modes::{human_size, permission_mode_to_str, ToPath, Users};

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
            Self::SymbolicLink(is_valid_symlink(filepath))
        } else {
            Self::NormalFile
        }
    }
    /// Returns the expected first symbol from `ln -l` line.
    /// d for directory, s for socket, . for file, c for char device,
    /// b for block, l for links.
    pub fn dir_symbol(&self) -> char {
        match self {
            Self::Fifo => 'p',
            Self::Socket => 's',
            Self::Directory => 'd',
            Self::NormalFile => '.',
            Self::CharDevice => 'c',
            Self::BlockDevice => 'b',
            Self::SymbolicLink(_) => 'l',
        }
    }

    fn sortable_char(&self) -> char {
        match self {
            Self::Directory => 'a',
            Self::NormalFile => 'b',
            Self::SymbolicLink(_) => 'c',
            Self::BlockDevice => 'd',
            Self::CharDevice => 'e',
            Self::Socket => 'f',
            Self::Fifo => 'g',
        }
    }

    pub fn long_description(&self) -> &'static str {
        match self {
            Self::Fifo => "fifo",
            Self::Socket => "socket",
            Self::Directory => "directory",
            Self::NormalFile => "normal file",
            Self::CharDevice => "char device",
            Self::BlockDevice => "block device",
            Self::SymbolicLink(_) => "symbolic link",
        }
    }

    #[rustfmt::skip]
    pub fn size_description(&self) -> &'static str {
        match self {
            Self::Fifo              => "Size: ",
            Self::Socket            => "Size: ",
            Self::Directory         => "Elements:",
            Self::NormalFile        => "Size: ",
            Self::CharDevice        => "Major,Minor:",
            Self::BlockDevice       => "Major,Minor:",
            Self::SymbolicLink(_)   => "Size: ",
        }
    }

    pub fn is_normal_file(&self) -> bool {
        matches!(self, Self::NormalFile)
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
    EntryCount(u64),
    /// Use for CharDevice and BlockDevice.
    /// It's the major & minor driver versions.
    MajorMinor((u8, u8)),
}

impl std::fmt::Display for SizeColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Size(bytes) => write!(f, "   {hs}", hs = human_size(*bytes)),
            Self::EntryCount(count) => write!(f, "{hs:>6} ", hs = count),
            Self::MajorMinor((major, minor)) => write!(f, "{major:>3},{minor:<3}"),
        }
    }
}

impl SizeColumn {
    fn new(size: u64, metadata: &Metadata, file_kind: &FileKind<Valid>) -> Self {
        match file_kind {
            FileKind::Directory => Self::EntryCount(size),
            FileKind::CharDevice | FileKind::BlockDevice => Self::MajorMinor(major_minor(metadata)),
            _ => Self::Size(size),
        }
    }

    pub fn trimed(&self) -> String {
        format!("{self}").trim().to_owned()
    }
}

/// Infos about a file
/// We read and keep tracks every displayable information about
/// a file.
#[derive(Clone, Debug)]
pub struct FileInfo {
    /// Full path of the file
    pub path: Arc<path::Path>,
    /// Filename
    pub filename: Arc<str>,
    /// File size as a `String`, already human formated.
    /// For char devices and block devices we display major & minor like ls.
    pub size_column: SizeColumn,
    /// True size of a file, not formated
    pub true_size: u64,
    /// Owner name of the file.
    pub owner: Arc<str>,
    /// Group name of the file.
    pub group: Arc<str>,
    /// System time of last modification
    pub system_time: Arc<str>,
    /// What kind of file is this ?
    pub file_kind: FileKind<Valid>,
    /// Extension of the file. `""` for a directory.
    pub extension: Arc<str>,
}

impl FileInfo {
    pub fn new(path: &path::Path, users: &Users) -> Result<Self> {
        let filename = extract_filename(path)?;
        let metadata = symlink_metadata(path)?;
        let true_size = true_size(path, &metadata);
        let path = Arc::from(path);
        let owner = extract_owner(&metadata, users);
        let group = extract_group(&metadata, users);
        let system_time = extract_datetime(metadata.modified()?)?;
        let file_kind = FileKind::new(&metadata, &path);
        let size_column = SizeColumn::new(true_size, &metadata, &file_kind);
        let extension = extract_extension(&path).into();

        Ok(FileInfo {
            path,
            filename,
            size_column,
            true_size,
            owner,
            group,
            system_time,
            file_kind,
            extension,
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
        file_info.filename = Arc::from(filename);
        Ok(file_info)
    }

    /// Symlink metadata of the file.
    /// Doesn't follow the symlinks.
    /// Correspond to `lstat` function on Linux.
    /// See [`std::fs::symlink_metadata`].
    ///
    /// # Errors
    ///
    /// Could return an error if the file doesn't exist or if the user can't stat it.
    pub fn metadata(&self) -> std::io::Result<std::fs::Metadata> {
        symlink_metadata(&self.path)
    }

    /// Returns the Inode number.
    ///
    /// Returns 0 if the metadata can't be read.
    pub fn ino(&self) -> u64 {
        self.metadata()
            .map(|metadata| metadata.ino())
            .unwrap_or_default()
    }

    /// String representation of file permissions
    pub fn permissions(&self) -> Result<Arc<str>> {
        Ok(permission_mode_to_str(self.metadata()?.mode()))
    }

    /// A formated filename where the "kind" of file
    /// (directory, char device, block devive, fifo, socket, normal)
    /// is prepend to the name, allowing a "sort by kind" method.
    pub fn kind_format(&self) -> String {
        format!(
            "{c}{filename}",
            c = self.file_kind.sortable_char(),
            filename = self.filename
        )
    }
    /// Format the file line.
    /// Since files can have different owners in the same directory, we need to
    /// know the maximum size of owner column for formatting purpose.
    #[inline]
    pub fn format_metadata(&self, owner_col_width: usize, group_col_width: usize) -> String {
        let mut repr = self.format_base(owner_col_width, group_col_width);
        repr.push(' ');
        repr.push_str(&self.filename);
        self.expand_symlink(&mut repr);
        repr
    }

    fn expand_symlink(&self, repr: &mut String) {
        if let FileKind::SymbolicLink(_) = self.file_kind {
            match std::fs::read_link(&self.path) {
                Ok(dest) if dest.exists() => {
                    repr.push_str(&format!(" -> {dest}", dest = dest.display()))
                }
                _ => repr.push_str("  broken link"),
            }
        }
    }

    pub fn format_no_group(&self, owner_col_width: usize) -> String {
        let owner = format!("{owner:.owner_col_width$}", owner = self.owner,);
        let permissions = self
            .permissions()
            .unwrap_or_else(|_| Arc::from("?????????"));
        format!(
            "{dir_symbol}{permissions} {file_size} {owner:<owner_col_width$} {system_time}",
            dir_symbol = self.dir_symbol(),
            file_size = self.size_column,
            system_time = self.system_time,
        )
    }

    pub fn format_no_permissions(&self, owner_col_width: usize) -> String {
        let owner = format!("{owner:.owner_col_width$}", owner = self.owner,);
        format!(
            "{file_size} {owner:<owner_col_width$} {system_time}",
            file_size = self.size_column,
            system_time = self.system_time,
        )
    }

    pub fn format_no_owner(&self) -> String {
        format!("{file_size}", file_size = self.size_column)
    }

    pub fn format_base(&self, owner_col_width: usize, group_col_width: usize) -> String {
        let owner = format!("{owner:.owner_col_width$}", owner = self.owner,);
        let group = format!("{group:.group_col_width$}", group = self.group,);
        let permissions = self
            .permissions()
            .unwrap_or_else(|_| Arc::from("?????????"));
        format!(
            "{dir_symbol}{permissions} {file_size} {owner:<owner_col_width$} {group:<group_col_width$} {system_time}",
            dir_symbol = self.dir_symbol(),
            file_size = self.size_column,
            system_time = self.system_time,
        )
    }
    /// Format the metadata line, without the filename.
    /// Owned & Group have fixed width of 6.
    pub fn format_no_filename(&self) -> String {
        self.format_base(6, 6)
    }

    pub fn dir_symbol(&self) -> char {
        self.file_kind.dir_symbol()
    }

    /// True iff the file is hidden (aka starts with a '.').
    pub fn is_hidden(&self) -> bool {
        self.filename.starts_with('.')
    }

    pub fn is_dir(&self) -> bool {
        matches!(self.file_kind, FileKind::Directory)
    }

    /// True iff the parent of the file is root.
    /// It's also true for the root folder itself.
    fn is_root_or_parent_is_root(&self) -> bool {
        match self.path.as_ref().parent() {
            None => true,
            Some(parent) => parent == path::Path::new("/"),
        }
    }

    /// Formated proper name.
    /// "/ " for `.`
    pub fn filename_without_dot_dotdot(&self) -> String {
        let sep = if self.is_root_or_parent_is_root() {
            ""
        } else {
            "/"
        };
        match self.filename.as_ref() {
            "." => format!("{sep} "),
            ".." => self.filename_without_dotdot(),
            _ => format!("{sep}{name} ", name = self.filename,),
        }
    }

    fn filename_without_dotdot(&self) -> String {
        let Ok(filename) = extract_filename(&self.path) else {
            return "/ ".to_string();
        };
        format!("/{filename} ")
    }

    #[inline]
    pub fn style(&self) -> Style {
        if matches!(self.file_kind, FileKind::NormalFile) {
            return extension_color(&self.extension).into();
        }
        let styles = FILE_STYLES.get().expect("Colors should be set");
        match self.file_kind {
            FileKind::Directory => styles.directory,
            FileKind::BlockDevice => styles.block,
            FileKind::CharDevice => styles.char,
            FileKind::Fifo => styles.fifo,
            FileKind::Socket => styles.socket,
            FileKind::SymbolicLink(true) => styles.symlink,
            FileKind::SymbolicLink(false) => styles.broken,
            _ => unreachable!("Should be done already"),
        }
    }
}

/// True if the file isn't hidden.
pub fn is_not_hidden(entry: &DirEntry) -> Result<bool> {
    let is_hidden = !entry
        .file_name()
        .to_str()
        .context("Couldn't read filename")?
        .starts_with('.');
    Ok(is_hidden)
}

fn extract_filename(path: &path::Path) -> Result<Arc<str>> {
    let s = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .context(format!("Couldn't read filename of {p}", p = path.display()))?;
    Ok(Arc::from(s))
}

/// Returns the modified time.
pub fn extract_datetime(time: std::time::SystemTime) -> Result<Arc<str>> {
    let datetime: DateTime<Local> = time.into();
    Ok(Arc::from(
        format!("{}", datetime.format("%Y/%m/%d %T")).as_str(),
    ))
}

/// Reads the owner name and returns it as a string.
/// If it's not possible to get the owner name (happens if the owner exists on a remote machine but not on host),
/// it returns the uid as a  `Result<String>`.
fn extract_owner(metadata: &Metadata, users: &Users) -> Arc<str> {
    match users.get_user_by_uid(metadata.uid()) {
        Some(name) => Arc::from(name.as_str()),
        None => Arc::from(format!("{}", metadata.uid()).as_str()),
    }
}

/// Reads the group name and returns it as a string.
/// If it's not possible to get the group name (happens if the group exists on a remote machine but not on host),
/// it returns the gid as a  `Result<String>`.
fn extract_group(metadata: &Metadata, users: &Users) -> Arc<str> {
    match users.get_group_by_gid(metadata.gid()) {
        Some(name) => Arc::from(name.as_str()),
        None => Arc::from(format!("{}", metadata.gid()).as_str()),
    }
}

/// Size of a file, number of entries of a directory
fn true_size(path: &path::Path, metadata: &Metadata) -> u64 {
    if path.is_dir() {
        count_entries(path).unwrap_or_default()
    } else {
        extract_file_size(metadata)
    }
}

/// Returns the file size.
fn extract_file_size(metadata: &Metadata) -> u64 {
    metadata.len()
}

/// Number of elements of a directory.
///
/// # Errors
///
/// Will fail if the provided path isn't a directory
/// or doesn't exist.
fn count_entries(path: &path::Path) -> Result<u64> {
    Ok(std::fs::read_dir(path)?.count() as u64)
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

/// true iff the path is a valid symlink (pointing to an existing file).
fn is_valid_symlink(path: &path::Path) -> bool {
    matches!(std::fs::read_link(path), Ok(dest) if dest.exists())
}

impl ToPath for FileInfo {
    fn to_path(&self) -> &path::Path {
        self.path.as_ref()
    }
}
