pub struct Input {
    pub string: String,
    pub cursor_index: usize,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            string: "".to_owned(),
            cursor_index: 0,
        }
    }
}

impl Input {
    pub fn reset(&mut self) {
        self.string.clear();
        self.cursor_index = 0;
    }

    pub fn cursor_start(&mut self) {
        self.cursor_index = 0;
    }

    pub fn cursor_end(&mut self) {
        self.cursor_index = self.string.len();
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_index > 0 {
            self.cursor_index -= 1
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_index < self.string.len() {
            self.cursor_index += 1
        }
    }

    pub fn delete_char_left(&mut self) {
        if self.cursor_index > 0 && !self.string.is_empty() {
            self.string.remove(self.cursor_index - 1);
            self.cursor_index -= 1;
        }
    }

    pub fn delete_chars_right(&mut self) {
        self.string = self
            .string
            .chars()
            .into_iter()
            .take(self.cursor_index)
            .collect();
    }

    pub fn insert(&mut self, c: char) {
        self.string.insert(self.cursor_index, c);
        self.cursor_index += 1
    }

    pub fn replace(&mut self, content: String) {
        self.string = content
    }
}
