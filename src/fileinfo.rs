use chrono::offset::Local;
use chrono::DateTime;
use std::fs::{metadata, read_dir, DirEntry};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path;

use users::get_user_by_uid;

#[derive(Debug, Clone)]
pub enum SortBy {
    Filename,
    Date,
    Size,
    Extension,
}

impl SortBy {
    pub fn by_key(&self, file: &FileInfo) -> String {
        match *self {
            Self::Filename => file.filename.clone(),
            Self::Date => file.system_time.clone(),
            Self::Size => file.file_size.clone(),
            Self::Extension => file.extension.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FileKind {
    NormalFile,
    Directory,
    BlockDevice,
    CharDevice,
    Fifo,
    Socket,
    SymbolicLink,
}

impl FileKind {
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
    fn extract_dir_symbol(&self) -> String {
        match self {
            FileKind::Fifo => "p".into(),
            FileKind::Socket => "s".into(),
            FileKind::Directory => "d".into(),
            FileKind::NormalFile => ".".into(),
            FileKind::CharDevice => "c".into(),
            FileKind::BlockDevice => "b".into(),
            FileKind::SymbolicLink => "l".into(),
        }
    }
}

#[derive(Clone)]
pub struct FileInfo {
    pub path: path::PathBuf,
    pub filename: String,
    pub file_size: String,
    pub dir_symbol: String,
    pub permissions: String,
    pub owner: String,
    pub system_time: String,
    pub is_selected: bool,
    pub file_kind: FileKind,
    pub extension: String,
}

impl FileInfo {
    pub fn new(direntry: &DirEntry) -> Result<FileInfo, &'static str> {
        let path = direntry.path();
        let filename = extract_filename(direntry);
        let file_size = human_size(extract_file_size(direntry));
        let permissions = extract_permissions_string(direntry);
        let owner = extract_owner(direntry);
        let system_time = extract_datetime(direntry);
        let is_selected = false;

        let file_kind = FileKind::new(direntry);
        let dir_symbol = file_kind.extract_dir_symbol();
        let extension = get_extension_from_filename(&filename).unwrap_or("").into();

        Ok(FileInfo {
            path,
            filename,
            file_size,
            dir_symbol,
            permissions,
            owner,
            system_time,
            is_selected,
            file_kind,
            extension,
        })
    }

    fn format(&self, owner_col_width: usize) -> String {
        format!(
            "{dir_symbol}{permissions} {file_size} {owner:<owner_col_width$} {system_time} {filename}",
            dir_symbol = self.dir_symbol,
            permissions = self.permissions,
            file_size = self.file_size,
            owner = self.owner,
            owner_col_width = owner_col_width,
            system_time = self.system_time,
            filename = self.filename,
        )
    }
}

impl FileInfo {
    pub fn select(&mut self) {
        self.is_selected = true;
    }

    pub fn unselect(&mut self) {
        self.is_selected = false;
    }
}

#[derive(Clone)]
pub struct PathContent {
    pub path: path::PathBuf,
    pub files: Vec<FileInfo>,
    pub selected: usize,
    pub show_hidden: bool,
    pub sort_by: SortBy,
}

impl PathContent {
    pub fn new(path: path::PathBuf, show_hidden: bool) -> Self {
        let mut files: Vec<FileInfo> = read_dir(&path)
            .unwrap_or_else(|_| {
                eprintln!(
                    "File does not exists {}",
                    path.to_str().unwrap_or("unreadable path")
                );
                std::process::exit(2)
            })
            .filter(|r| {
                show_hidden
                    || match r {
                        Ok(e) => is_not_hidden(e),
                        Err(_) => false,
                    }
            })
            .map(|direntry| FileInfo::new(&direntry.unwrap()).unwrap())
            .collect();
        let sort_by = SortBy::Filename;
        files.sort_by_key(|file| sort_by.by_key(file));
        let selected: usize = 0;
        if !files.is_empty() {
            files[selected].select();
        }

        Self {
            path,
            files,
            selected,
            show_hidden,
            sort_by,
        }
    }

    pub fn sort(&mut self) {
        self.files.sort_by_key(|file| self.sort_by.by_key(file));
    }

    fn owner_column_width(&self) -> usize {
        let mut owner_size_btreeset = std::collections::BTreeSet::new();
        for file in self.files.iter() {
            owner_size_btreeset.insert(file.owner.len());
        }
        *owner_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    pub fn strings(&self) -> Vec<String> {
        let owner_size = self.owner_column_width();
        self.files
            .iter()
            .map(|fileinfo| fileinfo.format(owner_size))
            .collect()
    }

    pub fn select_next(&mut self) {
        if self.selected < self.files.len() - 1 {
            self.files[self.selected].unselect();
            self.selected += 1;
            self.files[self.selected].select();
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.files[self.selected].unselect();
            self.selected -= 1;
            self.files[self.selected].select();
        }
    }

    pub fn select_index(&mut self, index: usize) {
        self.files[self.selected].unselect();
        self.files[index].select();
        self.selected = index;
    }

    pub fn reset_files(&mut self) {
        self.files = read_dir(&self.path)
            .unwrap_or_else(|_| panic!("Couldn't traverse path {:?}", &self.path))
            .filter(|r| {
                self.show_hidden
                    || match r {
                        Ok(e) => is_not_hidden(e),
                        Err(_) => false,
                    }
            })
            .map(|direntry| FileInfo::new(&direntry.unwrap()).unwrap())
            .collect();
        self.files.sort_by_key(|file| file.filename.clone());
        self.selected = 0;
        if !self.files.is_empty() {
            self.files[0].select();
        }
    }
}

fn is_not_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| !s.starts_with('.'))
        .unwrap_or(false)
}
fn extract_datetime(direntry: &DirEntry) -> String {
    let system_time = direntry.metadata().unwrap().modified();
    let datetime: DateTime<Local> = system_time.unwrap().into();
    format!("{}", datetime.format("%d/%m/%Y %T"))
}

fn extract_filename(direntry: &DirEntry) -> String {
    direntry.file_name().into_string().unwrap()
}

fn extract_permissions_string(direntry: &DirEntry) -> String {
    match metadata(direntry.path()) {
        Ok(metadata) => {
            let mode = metadata.mode() & 511;
            let s_o = convert_octal_mode(mode >> 6);
            let s_g = convert_octal_mode((mode >> 3) & 7);
            let s_a = convert_octal_mode(mode & 7);

            [s_o, s_g, s_a].join("")
        }
        Err(_) => String::from("---"),
    }
}

fn convert_octal_mode(mode: u32) -> String {
    let rwx = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
    String::from(rwx[(mode & 7_u32) as usize])
}

fn extract_owner(direntry: &DirEntry) -> String {
    match metadata(direntry.path()) {
        Ok(metadata) => String::from(
            get_user_by_uid(metadata.uid())
                .unwrap()
                .name()
                .to_str()
                .unwrap(),
        ),
        Err(_) => String::from(""),
    }
}

fn extract_file_size(direntry: &DirEntry) -> u64 {
    match direntry.path().metadata() {
        Ok(size) => size.len(),
        Err(_) => 0,
    }
}

fn human_size(bytes: u64) -> String {
    let size = ["", "k", "M", "G", "T", "P", "E", "Z", "Y"];
    let factor = (bytes.to_string().chars().count() as u64 - 1) / 3_u64;
    let human_size = format!(
        "{:>3}{:<1}",
        bytes / (1024_u64).pow(factor as u32),
        size[factor as usize]
    );
    human_size
}

fn get_extension_from_filename(filename: &str) -> Option<&str> {
    path::Path::new(filename)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
}
