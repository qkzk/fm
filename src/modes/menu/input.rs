use unicode_segmentation::UnicodeSegmentation;

/// Holds the chars typed by the user and the cursor position.
/// Methods allow mutation of this content and movement of the cursor.
#[derive(Clone, Default)]
pub struct Input {
    /// The input typed by the user
    chars: Vec<String>,
    /// The index of the cursor in that string
    cursor_index: usize,
}

impl Input {
    const RIGHT_BUFFER: usize = 4;

    /// Empty the content and move the cursor to start.
    pub fn reset(&mut self) {
        self.chars.clear();
        self.cursor_index = 0;
    }

    /// Current index of the cursor
    #[must_use]
    pub fn index(&self) -> usize {
        self.cursor_index
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Returns the content typed by the user as a String.
    #[must_use]
    pub fn string(&self) -> String {
        self.chars.join("")
    }

    pub fn left_index(&self, avail: usize) -> usize {
        if avail < self.len() {
            (self.index() + Self::RIGHT_BUFFER).saturating_sub(avail)
        } else {
            0
        }
    }

    pub fn display_index(&self, avail: usize) -> usize {
        self.index().saturating_sub(self.left_index(avail))
    }

    pub fn string_in_rect(&self, avail: usize) -> String {
        self.chars
            .iter()
            .skip(self.left_index(avail))
            .take(avail)
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join("")
    }

    /// Returns a string of * for every char typed.
    #[must_use]
    pub fn password(&self) -> String {
        "*".repeat(self.len())
    }

    /// Insert an utf-8 char into the input at cursor index.
    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor_index, String::from(c));
        self.cursor_index += 1;
    }

    pub fn insert_string(&mut self, pasted: &str) {
        UnicodeSegmentation::graphemes(pasted, true)
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| (*s).to_string())
            .for_each(|s| {
                self.chars.insert(self.cursor_index, s);
                self.cursor_index += 1;
            })
    }
    /// Move the cursor to the start
    pub fn cursor_start(&mut self) {
        self.cursor_index = 0;
    }

    /// Move the cursor to the end
    pub fn cursor_end(&mut self) {
        self.cursor_index = self.len();
    }

    /// Move the cursor left if possible
    pub fn cursor_left(&mut self) {
        if self.cursor_index > 0 {
            self.cursor_index -= 1;
        }
    }

    /// Move the cursor right if possible
    pub fn cursor_right(&mut self) {
        if self.cursor_index < self.len() {
            self.cursor_index += 1;
        }
    }

    /// Move the cursor to the given index, limited to the length of the input.
    ///
    /// Used when the user click on the input string.
    pub fn cursor_move(&mut self, index: usize) {
        if index <= self.len() {
            self.cursor_index = index
        } else {
            self.cursor_end()
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
            .map(std::string::ToString::to_string)
            .collect();
    }

    pub fn delete_line(&mut self) {
        self.chars = vec![];
        self.cursor_index = 0;
    }

    /// Deletes left symbols until a separator is reached or the start of the line
    pub fn delete_left(&mut self) {
        while self.cursor_index > 0 {
            self.chars.remove(self.cursor_index.saturating_sub(1));
            self.cursor_index -= 1;
            if self.is_empty() || is_separator(&self.chars[self.cursor_index.saturating_sub(1)]) {
                break;
            }
        }
    }

    /// Replace the content with the new content.
    /// Put the cursor at the end.
    ///
    /// To avoid splitting graphemes at wrong place, the new content is read
    /// as Unicode Graphemes with
    /// ```rust
    /// unicode_segmentation::UnicodeSegmentation::graphemes(content, true)
    /// ```
    /// See [`UnicodeSegmentation`] for more information.
    pub fn replace(&mut self, content: &str) {
        self.chars = UnicodeSegmentation::graphemes(content, true)
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        self.cursor_end();
    }
}

fn is_separator(word: &str) -> bool {
    matches!(
        word,
        " " | "/"
            | "\\"
            | "."
            | ","
            | ";"
            | "!"
            | "?"
            | "%"
            | "_"
            | "-"
            | "+"
            | "*"
            | "("
            | ")"
            | "{"
            | "}"
            | "["
            | "]"
    )
}
