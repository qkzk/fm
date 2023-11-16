use std::path::{Path, PathBuf};

use crate::impl_selectable_content;

type DoublePB = (PathBuf, PathBuf);

/// A stack of visited paths.
/// We save the last folder and the selected file every time a `PatchContent` is updated.
/// We also ensure not to save the same pair multiple times.
#[derive(Default, Clone)]
pub struct History {
    pub content: Vec<DoublePB>,
    pub index: usize,
}

impl History {
    /// Add a new path and a selected file in the stack, without duplicates, and select the last
    /// one.
    pub fn push(&mut self, path: &Path, file: &Path) {
        let pair = (path.to_owned(), file.to_owned());
        if !self.content.contains(&pair) {
            self.content.push(pair);
            self.index = self.len() - 1
        }
    }

    /// Drop the last visited paths from the stack, after the selected one.
    /// Used to go back a few steps in time.
    pub fn drop_queue(&mut self) {
        if self.is_empty() {
            return;
        }
        let final_length = self.len() - self.index + 1;
        self.content.truncate(final_length);
        if self.is_empty() {
            self.index = 0
        } else {
            self.index = self.len() - 1
        }
    }

    /// True iff the last element of the stack has the same
    /// path as the one given.
    /// Doesn't check the associated file.
    /// false if the stack is empty.
    pub fn is_this_the_last(&self, path: &Path) -> bool {
        if self.is_empty() {
            return false;
        }
        self.content[self.len() - 1].0 == path
    }
}

impl_selectable_content!(DoublePB, History);
