use chrono::offset::Local;
use chrono::DateTime;
use std::fs::{canonicalize, metadata, read_dir, DirEntry};
use std::os::unix::fs::MetadataExt;
// use std::os::unix::fs::PermissionsExt;
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
}

impl FileInfo {
    pub fn new(direntry: &DirEntry) -> Result<FileInfo, &'static str> {
        let filename = extract_filename(&direntry);
        let file_size = human_size(extract_file_size(&direntry));
        let dir_symbol = extract_dir_symbol(&direntry);
        let permissions = extract_permissions_string(&direntry);
        let owner = extract_username(&direntry);
        let system_time = extract_datetime(&direntry);
        let is_selected = false;
        let is_dir = direntry.path().is_dir();

        Ok(FileInfo {
            filename,
            file_size,
            dir_symbol,
            permissions,
            owner,
            system_time,
            is_selected,
            is_dir,
        })
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

pub fn expand(path: &path::Path) -> Result<path::PathBuf, std::io::Error> {
    canonicalize(path)
}

#[derive(Clone)]
pub struct PathContent<'a> {
    pub path: &'a path::Path,
    pub files: Vec<FileInfo>,
    pub selected: usize,
}

impl<'a> PathContent<'a> {
    pub fn new(path: &'a path::Path) -> Self {
        let mut files: Vec<FileInfo> = read_dir(path)
            .expect(&format!("Couldn't traverse path {:?}", path))
            .map(|direntry| FileInfo::new(&direntry.unwrap()).unwrap())
            .collect();
        let selected: usize = 0;
        files[selected].select();

        Self {
            path,
            files,
            selected,
        }
    }

    pub fn strings(&self) -> Vec<String> {
        let content =
            read_dir(self.path).expect(&format!("Couldn't traverse path {:?}", &self.path));
        content
            .map(|direntry| {
                let fileinfo = FileInfo::new(&direntry.unwrap()).unwrap();
                format!(
                    "{}{} {} {} {} {}",
                    fileinfo.dir_symbol,
                    fileinfo.permissions,
                    fileinfo.file_size,
                    fileinfo.owner,
                    fileinfo.system_time,
                    fileinfo.filename,
                )
            })
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

    // pub fn child(&self) -> Self {
    //     let mut pb = self.path.to_path_buf();
    //     pb.push(self.files[self.selected].filename);
    //     let p = pb.as_path();
    //     // let mut files: Vec<FileInfo> = read_dir(p)
    //     //     .expect(&format!("Couldn't traverse path {:?}", p))
    //     //     .map(|direntry| FileInfo::new(&direntry.unwrap()).unwrap())
    //     //     .collect();
    //     // let selected: usize = 0;
    //     PathContent::new(p)
    //     // files[selected].select();
    //     // // self.path = p;
    //     // self.files = files;
    //     // self.selected = selected;
    // }
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
//
// fn get_path_content(path: &path::Path) -> Result<Vec<path::PathBuf>, Box<dyn Error>> {
//     let mut entries = read_dir(&path)?
//         .map(|res| res.map(|e| e.path()))
//         .collect::<Result<Vec<_>, io::Error>>()?;
//
//     entries.sort();
//
//     Ok(entries)
// }

fn convert_octal_mode(mode: u32) -> String {
    let rwx = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
    String::from(rwx[(mode & 7 as u32) as usize])
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
    // let meta = metadata(direntry.path());
    // let owner_id: u32 = meta.unwrap().uid();
    // let user = get_user_by_uid(owner_id).unwrap();
    // String::from(user.name().to_str().unwrap())
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
    let factor = (bytes.to_string().chars().count() as u64 - 1) / 3 as u64;
    let human_size = format!(
        "{:>3}{:<1}",
        bytes / (1024 as u64).pow(factor as u32),
        size[factor as usize]
    );
    human_size
}
