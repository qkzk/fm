use unicode_segmentation::UnicodeSegmentation;

/// Holds the chars typed by the user and the cursor position.
/// Methods allow mutation of this content and movement of the cursor.
#[derive(Clone, Default)]
pub struct Input {
    /// The input typed by the user
    chars: Vec<String>,
    /// The index of the cursor in that string
    pub cursor_index: usize,
}

impl Input {
    /// Empty the content and move the cursor to start.
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
        self.chars = self
            .chars
            .iter()
            .take(self.cursor_index)
            .map(|s| s.to_string())
            .collect();
    }

    /// Returns the content typed by the user as a String.
    pub fn string(&self) -> String {
        self.chars.join("")
    }

    /// Returns a string of * for every char typed.
    pub fn password(&self) -> String {
        self.chars.iter().map(|_| '*').collect()
    }

    /// Insert an utf-8 char into the input at cursor index.
    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor_index, String::from(c));
        self.cursor_index += 1
    }

    /// Clear the chars in input
    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor_index = 0;
    }

    /// Replace the content with the new content.
    /// Put the cursor at the end.
    ///
    /// To avoid splitting graphemes at wrong place, the new content is read
    /// as Unicode Graphemes with
    /// ```rust
    /// unicode_segmentation::UnicodeSegmentation::graphemes(content, true)
    /// ```
    pub fn replace(&mut self, content: &str) {
        self.chars = UnicodeSegmentation::graphemes(content, true)
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| s.to_string())
            .collect();
        self.cursor_index = self.len()
    }
}