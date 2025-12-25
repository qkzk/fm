use std::{
    cmp::min,
    path::{Path, PathBuf},
};

use crate::common::tilde;
use crate::io::{DrawMenu, Extension};
use crate::modes::extract_extension;
use crate::{impl_content, impl_selectable};

/// The flagged files and an index, allowing navigation when the flagged files are displayed.
/// We record here every flagged file by its path, allowing deletion, renaming, copying, moving and other actions.
#[derive(Default)]
pub struct Flagged {
    /// Contains the different flagged files.
    /// It's basically a `Set` (of whatever kind) and insertion would be faster
    /// using a set.
    /// Iteration is faster with a vector and we need a vector to use the common trait
    /// `SelectableContent` which can be implemented with a macro.
    /// We use binary search in every non standard method (insertion, removal, search).
    pub content: Vec<PathBuf>,
    /// The index of the selected file. Used to jump.
    pub index: usize,
}

impl Flagged {
    pub fn update(&mut self, content: Vec<PathBuf>) {
        self.content = content;
        self.content.sort();
        self.index = 0;
    }

    pub fn extend(&mut self, mut content: Vec<PathBuf>) {
        self.content.append(&mut content);
        self.content.sort();
        self.index = 0;
    }

    pub fn clear(&mut self) {
        self.content = vec![];
        self.index = 0;
    }

    pub fn remove_selected(&mut self) {
        self.content.remove(self.index);
        self.index = self.index.saturating_sub(1);
    }

    /// Push a new path into the content.
    /// We maintain the content sorted and it's used to make `contains` faster.
    pub fn push(&mut self, path: PathBuf) {
        let Err(pos) = self.content.binary_search(&path) else {
            return;
        };
        self.content.insert(pos, path);
    }

    /// Toggle the flagged status of a path.
    /// Remove the path from the content if it's flagged, flag it if it's not.
    /// The implantation assumes the content to be sorted.
    pub fn toggle(&mut self, path: &Path) {
        let path = path.to_path_buf();
        match self.content.binary_search(&path) {
            Ok(pos) => self.remove_index(pos),
            Err(pos) => self.content.insert(pos, path),
        }
    }

    fn remove_index(&mut self, index: usize) {
        self.content.remove(index);
        if self.index >= self.len() {
            self.index = self.index.saturating_sub(1);
        }
    }

    /// True if the `path` is flagged.
    /// Since we maintain the content sorted, we can use a binary search and
    /// compensate a little bit with using a vector instead of a set.
    #[inline]
    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.content.binary_search(&path.to_path_buf()).is_ok()
    }

    /// Returns a vector of path which are present in the current directory but ARE NOT the current dir.
    /// This prevents the current directory (or root path in tree display mode) to be altered by bulk.
    #[inline]
    #[must_use]
    pub fn in_dir(&self, dir: &Path) -> Vec<PathBuf> {
        self.content
            .iter()
            .filter(|p| *p != dir)
            .filter(|p| p.starts_with(dir))
            .map(|p| p.to_owned())
            .collect()
    }

    /// Returns a string with every path in content on a separate line.
    pub fn content_to_string(&self) -> String {
        self.content()
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<String>>()
            .join("\n")
    }

    pub fn replace_by_string(&mut self, files: String) {
        self.clear();
        files.lines().for_each(|f| {
            let p = PathBuf::from(tilde(f).as_ref());
            if p.exists() {
                self.push(p);
            }
        });
    }

    /// Returns the flagged files as a vector of strings
    pub fn as_strings(&self) -> Vec<String> {
        self.content
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect()
    }

    fn should_this_file_be_opened_in_neovim(&self, path: &Path) -> bool {
        matches!(Extension::matcher(extract_extension(path)), Extension::Text)
    }

    pub fn should_all_be_opened_in_neovim(&self) -> bool {
        self.content()
            .iter()
            .all(|path| self.should_this_file_be_opened_in_neovim(path))
    }

    /// Remove all files from flagged which doesn't exists.
    pub fn remove_non_existant(&mut self) {
        let non_existant_indices: Vec<usize> = self
            .content
            .iter()
            .enumerate()
            .filter(|(_index, path)| !path.exists())
            .map(|(index, _path)| index)
            .rev()
            .collect();
        for index in non_existant_indices.iter() {
            self.content.remove(*index);
        }
        self.index = min(self.index, self.len().saturating_sub(1))
    }
}

impl_selectable!(Flagged);
impl_content!(Flagged, PathBuf);

impl DrawMenu<PathBuf> for Flagged {}
