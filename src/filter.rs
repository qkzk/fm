use std::fmt::Display;

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
        let words: Vec<&str> = input.split_whitespace().collect();
        if words.is_empty() {
            return Self::All;
        }
        match words[0] {
            "d" => Self::Directory,
            "e" if words.len() > 1 => Self::Extension(words[1].to_owned()),
            "n" if words.len() > 1 => Self::Name(words[1].to_owned()),
            _ => Self::All,
        }
    }

    /// Apply the selected filter to the file list.
    /// It's a "key" used by the Filter method to hold the files matching this
    /// filter.
    pub fn filter_by(&self, fileinfo: &FileInfo) -> bool {
        match self {
            Self::Extension(ext) => Self::filter_by_extension(fileinfo, ext),
            Self::Name(filename) => Self::filter_by_name(fileinfo, filename),
            Self::Directory => Self::filter_directory(fileinfo),
            Self::All => true,
        }
    }

    fn filter_by_extension(fileinfo: &FileInfo, ext: &str) -> bool {
        fileinfo.extension == ext
    }

    fn filter_by_name(fileinfo: &FileInfo, filename: &str) -> bool {
        if let Ok(re) = Regex::new(filename) {
            re.is_match(&fileinfo.filename)
        } else {
            false
        }
    }

    fn filter_directory(fileinfo: &FileInfo) -> bool {
        matches!(fileinfo.file_kind, FileKind::Directory)
    }
}

/// Format the corresponding variant to be printed in the second line.
/// We display a simple line with a variant description and the typed filter (if any).
impl Display for FilterKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Directory => write!(f, "Filter: Directory only"),
            Self::Extension(s) => write!(f, "Filter: by extension \"{}\"", s),
            Self::Name(s) => write!(f, "Filter: by name \"{}\"", s),
            Self::All => write!(f, ""),
        }
    }
}
