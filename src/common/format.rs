use std::path::Path;

use crate::common::UtfWidth;

/// Shorten a path to be displayed in 50 chars or less.
/// Each element of the path is shortened if needed.
pub struct PathShortener {
    size: usize,
    path_str: String,
}

impl PathShortener {
    const MAX_PATH_ELEM_SIZE: usize = 80;

    pub fn path(path: &Path) -> Option<Self> {
        Some(Self {
            path_str: path.to_str()?.to_owned(),
            size: Self::MAX_PATH_ELEM_SIZE,
        })
    }

    pub fn with_size(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    pub fn shorten(self) -> String {
        if self.path_str.utf_width() < self.size {
            return self.path_str;
        }
        self.shorten_long_path()
    }

    fn shorten_long_path(self) -> String {
        let splitted_path: Vec<_> = self.path_str.split('/').collect();
        let size_per_elem = std::cmp::max(1, self.size / (splitted_path.len() + 1)) + 1;
        Self::elems(splitted_path, size_per_elem).join("/")
    }

    fn elems(splitted_path: Vec<&str>, size_per_elem: usize) -> Vec<&str> {
        splitted_path
            .iter()
            .filter_map(|p| Self::slice_long(p, size_per_elem))
            .collect()
    }

    fn slice_long(p: &str, size_per_elem: usize) -> Option<&str> {
        if p.len() <= size_per_elem {
            Some(p)
        } else {
            p.get(0..size_per_elem)
        }
    }
}
