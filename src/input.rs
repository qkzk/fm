/// Holds a string typed by the user and the cursor position.
/// Methods allow mutation of this string and movement of the cursor.
#[derive(Clone)]
pub struct Input {
    /// The input string typed by the user
    pub string: String,
    /// The index of the cursor in that string
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
    /// Empty the string and move the cursor to start.
    pub fn reset(&mut self) {
        self.string.clear();
        self.cursor_index = 0;
    }

    /// Move the cursor to the start
    pub fn cursor_start(&mut self) {
        self.cursor_index = 0;
    }

    /// Move the cursor to the end
    pub fn cursor_end(&mut self) {
        self.cursor_index = self.string.len();
    }

    /// Move the cursor left if possible
    pub fn cursor_left(&mut self) {
        if self.cursor_index > 0 {
            self.cursor_index -= 1
        }
    }

    /// Move the cursor right if possible
    pub fn cursor_right(&mut self) {
        if self.cursor_index < self.string.len() {
            self.cursor_index += 1
        }
    }

    /// Backspace, delete the char under the cursor and move left
    pub fn delete_char_left(&mut self) {
        if self.cursor_index > 0 && !self.string.is_empty() {
            self.string.remove(self.cursor_index - 1);
            self.cursor_index -= 1;
        }
    }

    /// Delete all chars right to the cursor
    pub fn delete_chars_right(&mut self) {
        self.string = self
            .string
            .chars()
            .into_iter()
            .take(self.cursor_index)
            .collect();
    }

    /// Insert an ASCII char into the string at cursor index.
    /// Non ascii chars aren't supported in FM since it's a pain
    /// to know where you're in the string.
    pub fn insert(&mut self, c: char) {
        if c.is_ascii() {
            self.string.insert(self.cursor_index, c);
            self.cursor_index += 1
        }
    }

    /// replace the string with the content
    pub fn replace(&mut self, content: String) {
        self.string = content
    }
}
