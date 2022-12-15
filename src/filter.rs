use regex::Regex;

use crate::fileinfo::{FileInfo, FileKind};

/// Different kinds of filters.
/// By extension, by name, only the directory or all files.
#[derive(Clone)]
pub enum FilterKind {
    Extension(String),
    Name(String),
    Directory,
    All,
}

impl FilterKind {
    /// Parse the input string into a filter.
    /// It shouldn't fail but use a `Filter::All` if the string isn't parsable;
    pub fn from_input(input: &str) -> Self {
        let words = input.split_whitespace().collect::<Vec<&str>>();
        if words.len() < 2 {
            return Self::All;
        }
        match words[0] {
            "d" => Self::Directory,
            "e" => Self::Extension(words[1].to_owned()),
            "n" => Self::Name(words[1].to_owned()),
            _ => Self::All,
        }
    }

    /// Apply the selected filter to the file list.
    /// It's a "key" used by the Filter method to hold the files matching this
    /// filter.
    pub fn filter_by(&self, fileinfo: &FileInfo) -> bool {
        match self {
            Self::Extension(ext) => Self::filter_by_ext(fileinfo, ext.clone()),
            Self::Name(filename) => Self::filter_by_name(fileinfo, filename.clone()),
            Self::Directory => Self::filter_directory(fileinfo),
            Self::All => true,
        }
    }

    fn filter_by_ext(fileinfo: &FileInfo, ext: String) -> bool {
        fileinfo.extension == ext
    }

    fn filter_by_name(fileinfo: &FileInfo, filename: String) -> bool {
        if let Ok(re) = Regex::new(&filename) {
            re.is_match(&fileinfo.filename)
        } else {
            false
        }
    }

    fn filter_directory(fileinfo: &FileInfo) -> bool {
        matches!(fileinfo.file_kind, FileKind::Directory)
    }
}
