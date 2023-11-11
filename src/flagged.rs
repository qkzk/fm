use std::path::{Path, PathBuf};

use crate::impl_selectable_content;

#[derive(Clone, Debug, Default)]
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
    /// Push a new path into the content.
    /// We maintain the content sorted and it's used to make `contains` faster.
    pub fn push(&mut self, path: PathBuf) {
        match self.content.binary_search(&path) {
            Ok(_) => (),
            Err(pos) => self.content.insert(pos, path),
        }
    }

    /// Toggle the flagged status of a path.
    /// Remove the path from the content if it's flagged, flag it if it's not.
    /// The implantation assumes the content to be sorted.
    pub fn toggle(&mut self, path: &Path) {
        let path_buf = path.to_path_buf();
        match self.content.binary_search(&path_buf) {
            Ok(pos) => {
                self.content.remove(pos);
            }
            Err(pos) => {
                self.content.insert(pos, path_buf);
            }
        }
    }

    /// Empty the flagged files.
    pub fn clear(&mut self) {
        self.content.clear()
    }

    /// True if the `path` is flagged.
    /// Since we maintain the content sorted, we can use a binary search and
    /// compensate a little bit with using a vector instead of a set.
    #[inline]
    pub fn contains(&self, path: &Path) -> bool {
        self.content.binary_search(&path.to_path_buf()).is_ok()
    }

    /// Returns a vector of path which are present in the current directory.
    pub fn filtered(&self, current_path: &Path) -> Vec<&Path> {
        self.content
            .iter()
            .filter(|p| p.starts_with(current_path))
            .map(|p| p.as_path())
            .collect()
    }

    /// Remove the selected file from the flagged files.
    pub fn remove_selected(&mut self) {
        self.content.remove(self.index);
        self.index = 0;
    }
}

impl_selectable_content!(PathBuf, Flagged);
