/// Holds a `Vec<String>` of possible completions and an `usize` index
/// showing where the user is in the vec.
pub struct Completion {
    pub proposals: Vec<String>,
    pub index: usize,
}

impl Completion {
    /// Creates a new `Completion` instance with empty proposals and index=0.
    fn new() -> Self {
        Self {
            proposals: vec![],
            index: 0,
        }
    }

    /// Move the index to next element, cycling to 0.
    /// Does nothing if the list is empty.
    pub fn next(&mut self) {
        if self.proposals.is_empty() {
            return;
        }
        self.index = (self.index + 1) % self.proposals.len()
    }

    /// Move the index to previous element, cycling to the last one.
    /// Does nothing if the list is empty.
    pub fn prev(&mut self) {
        if self.proposals.is_empty() {
            return;
        }
        if self.index > 0 {
            self.index -= 1
        } else {
            self.index = self.proposals.len() - 1
        }
    }

    /// Returns the currently selected proposition.
    /// Returns an empty string if `proposals` is empty.
    pub fn current_proposition(&self) -> String {
        if self.proposals.is_empty() {
            return "".to_owned();
        }
        self.proposals[self.index].to_owned()
    }

    /// Updates the proposition with a new `Vec`.
    /// Reset the index to 0.
    pub fn update(&mut self, proposals: Vec<String>) {
        self.index = 0;
        self.proposals = proposals;
    }

    /// Empty the proposals `Vec`.
    /// Reset the index.
    pub fn reset(&mut self) {
        self.index = 0;
        self.proposals.clear();
    }
}

impl Default for Completion {
    fn default() -> Self {
        Self::new()
    }
}
