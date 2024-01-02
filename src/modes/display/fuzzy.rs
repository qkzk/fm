use std::path::PathBuf;

use crate::{impl_content, impl_selectable};

#[derive(Debug, Default)]
pub struct Fuzzy {
    pub content: Vec<PathBuf>,
    pub index: usize,
}

impl Fuzzy {
    pub fn new(content: Vec<PathBuf>) -> Self {
        Self { content, index: 0 }
    }

    pub fn page_down(&mut self) {
        for _ in 0..10 {
            self.next()
        }
    }

    pub fn page_up(&mut self) {
        for _ in 0..10 {
            self.prev()
        }
    }
}

impl_content!(PathBuf, Fuzzy);
impl_selectable!(Fuzzy);
