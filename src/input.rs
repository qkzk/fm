/// Holds a string typed by the user and the cursor position.
/// Methods allow mutation of this string and movement of the cursor.
#[derive(Clone, Default)]
pub struct Input {
    /// The input string typed by the user
    chars: Vec<char>,
    /// The index of the cursor in that string
    pub cursor_index: usize,
}

impl Input {
    /// Empty the string and move the cursor to start.
    pub fn reset(&mut self) {
        self.chars.clear();
        self.cursor_index = 0;
    }

    /// Move the cursor to the start
    pub fn cursor_start(&mut self) {
        self.cursor_index = 0;
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Move the cursor to the end
    pub fn cursor_end(&mut self) {
        self.cursor_index = self.len();
    }

    /// Move the cursor left if possible
    pub fn cursor_left(&mut self) {
        if self.cursor_index > 0 {
            self.cursor_index -= 1
        }
    }

    /// Move the cursor right if possible
    pub fn cursor_right(&mut self) {
        if self.cursor_index < self.len() {
            self.cursor_index += 1
        }
    }

    /// Backspace, delete the char under the cursor and move left
    pub fn delete_char_left(&mut self) {
        if self.cursor_index > 0 && !self.chars.is_empty() {
            self.chars.remove(self.cursor_index - 1);
            self.cursor_index -= 1;
        }
    }

    /// Delete all chars right to the cursor
    pub fn delete_chars_right(&mut self) {
        self.chars = self.chars.iter().copied().take(self.cursor_index).collect();
    }

    /// Returns the string typed by the user.
    pub fn string(&self) -> String {
        self.chars.iter().collect()
    }

    /// Insert an utf-8 char into the string at cursor index.
    /// Non ascii chars aren't supported in FM since it's a pain
    /// to know where you're in the string.
    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor_index, c);
        self.cursor_index += 1
    }

    /// Clear the chars in input
    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor_index = 0;
    }

    /// replace the string with the content
    pub fn replace(&mut self, content: String) {
        self.chars = content.chars().collect();
        self.cursor_index = self.len()
    }
}
