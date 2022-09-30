pub struct Completion {
    pub proposals: Vec<String>,
    pub index: usize,
}

impl Completion {
    fn new() -> Self {
        Self {
            proposals: vec![],
            index: 0,
        }
    }

    pub fn next(&mut self) {
        if self.proposals.is_empty() {
            return;
        }
        self.index = (self.index + 1) % self.proposals.len()
    }

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

    pub fn current_proposition(&self) -> String {
        if self.proposals.is_empty() {
            return "".to_owned();
        }
        self.proposals[self.index].to_owned()
    }

    pub fn update(&mut self, proposals: Vec<String>) {
        self.index = 0;
        self.proposals = proposals;
    }

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
