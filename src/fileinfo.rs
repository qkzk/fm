use chrono::offset::Local;
use chrono::DateTime;
use std::fs::{metadata, read_dir, DirEntry};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path;

use users::get_user_by_uid;

#[derive(Clone)]
pub struct FileInfo {
    pub filename: String,
    pub file_size: String,
    pub dir_symbol: String,
    pub permissions: String,
    pub owner: String,
    pub system_time: String,
    pub is_selected: bool,
    pub is_dir: bool,
    pub is_block: bool,
    pub is_char: bool,
    pub is_fifo: bool,
    pub is_socket: bool,
}

impl FileInfo {
    pub fn new(direntry: &DirEntry) -> Result<FileInfo, &'static str> {
        let filename = extract_filename(direntry);
        let file_size = human_size(extract_file_size(direntry));
        let dir_symbol = extract_dir_symbol(direntry);
        let permissions = extract_permissions_string(direntry);
        let owner = extract_username(direntry);
        let system_time = extract_datetime(direntry);
        let is_selected = false;
        let is_dir = direntry.path().is_dir();

        let mut is_block: bool = false;
        let mut is_socket: bool = false;
        let mut is_char: bool = false;
        let mut is_fifo: bool = false;

        if let Ok(meta) = direntry.metadata() {
            is_block = meta.file_type().is_block_device();
            is_socket = meta.file_type().is_socket();
            is_char = meta.file_type().is_char_device();
            is_fifo = meta.file_type().is_fifo();
        }

        Ok(FileInfo {
            filename,
            file_size,
            dir_symbol,
            permissions,
            owner,
            system_time,
            is_selected,
            is_dir,
            is_block,
            is_char,
            is_fifo,
            is_socket,
        })
    }

    fn format(&self) -> String {
        format!(
            "{}{} {} {} {} {}",
            self.dir_symbol,
            self.permissions,
            self.file_size,
            self.owner,
            self.system_time,
            self.filename,
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
}

impl PathContent {
    pub fn new(path: path::PathBuf, show_hidden: bool) -> Self {
        let mut files: Vec<FileInfo> = read_dir(&path)
            .unwrap_or_else(|_| panic!("Couldn't traverse path {:?}", &path))
            .filter(|r| {
                show_hidden
                    || match r {
                        Ok(e) => is_not_hidden(e),
                        Err(_) => false,
                    }
            })
            .map(|direntry| FileInfo::new(&direntry.unwrap()).unwrap())
            .collect();
        files.sort_by_key(|file| file.filename.clone());
        let selected: usize = 0;
        files[selected].select();

        Self {
            path,
            files,
            selected,
            show_hidden,
        }
    }

    pub fn strings(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|fileinfo| fileinfo.format())
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
        self.files[0].select();
    }
}

fn is_not_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| !s.starts_with("."))
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

fn extract_username(direntry: &DirEntry) -> String {
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

fn extract_dir_symbol(direntry: &DirEntry) -> String {
    match metadata(direntry.path()) {
        Ok(path) => String::from(if path.is_dir() { "d" } else { "." }),
        Err(_) => String::from("."),
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
