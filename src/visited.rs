use std::path::PathBuf;

#[derive(Default)]
pub struct History {
    pub visited: Vec<PathBuf>,
    pub index: usize,
}

impl History {
    pub fn push(&mut self, path: &PathBuf) {
        if !self.visited.contains(path) {
            self.visited.push(path.to_owned());
            self.index = self.len() - 1
        }
    }

    pub fn is_empty(&self) -> bool {
        self.visited.is_empty()
    }

    pub fn len(&self) -> usize {
        self.visited.len()
    }

    pub fn next(&mut self) {
        if self.is_empty() {
            self.index = 0
        } else if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.len() - 1
        }
    }

    pub fn prev(&mut self) {
        if self.is_empty() {
            self.index = 0;
        } else {
            self.index = (self.index + 1) % self.len()
        }
    }

    pub fn selected(&self) -> Option<PathBuf> {
        if self.index < self.len() {
            Some(self.visited[self.index].clone())
        } else {
            None
        }
    }

    pub fn drop_queue(&mut self) {
        if self.is_empty() {
            return;
        }
        let final_length = self.len() - self.index;
        self.visited.truncate(final_length);
        if self.is_empty() {
            self.index = 0
        } else {
            self.index = self.len() - 1
        }
    }
}
