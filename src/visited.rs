use std::path::PathBuf;

use crate::indexed_vector::IndexedVector;

/// A Vec of pathbuf of visited files.
/// It's mostly used as a stack but we want to avoid multiple instances of the
/// same path and visit in a certain order.
/// A `BTreeSet` may be used instead.
/// We also need to drop the queue, which isn't easy with a BTreeSet...
#[derive(Default, Clone)]
pub struct History {
    /// The visited paths
    pub content: Vec<PathBuf>,
    /// The currently selected index. By default it's the last one.
    pub index: usize,
}

impl History {
    /// Add a new path in the stack, without duplicates, and select the last
    /// one.
    pub fn push(&mut self, path: &PathBuf) {
        if !self.content.contains(path) {
            self.content.push(path.to_owned());
            self.index = self.len() - 1
        }
    }

    /// Drop the last visited paths from the stack, after the selected one.
    /// Used to go back a few steps in time.
    pub fn drop_queue(&mut self) {
        if self.is_empty() {
            return;
        }
        let final_length = self.len() - self.index;
        self.content.truncate(final_length);
        if self.is_empty() {
            self.index = 0
        } else {
            self.index = self.len() - 1
        }
    }
}

impl IndexedVector<PathBuf> for History {
    /// True if nothing was visited. Shouldn't be the case...
    fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Number of visited paths.
    fn len(&self) -> usize {
        self.content.len()
    }

    /// Select the next visited path.
    fn next(&mut self) {
        if self.is_empty() {
            self.index = 0
        } else if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.len() - 1
        }
    }

    /// Select the previously visited path.
    fn prev(&mut self) {
        if self.is_empty() {
            self.index = 0;
        } else {
            self.index = (self.index + 1) % self.len()
        }
    }

    /// Returns the currently selected visited path.
    fn selected(&self) -> Option<&PathBuf> {
        if self.index < self.len() {
            Some(&self.content[self.index])
        } else {
            None
        }
    }
}
