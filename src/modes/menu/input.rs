use unicode_segmentation::UnicodeSegmentation;

/// Holds the chars typed by the user and the cursor position.
/// Methods allow mutation of this content and movement of the cursor.
#[derive(Clone, Default)]
pub struct Input {
    /// The input typed by the user
    symbols: Vec<String>,
    /// The index of the cursor in that string
    cursor_index: usize,
}

impl Input {
    const RIGHT_GAP: usize = 4;

    /// Empty the content and move the cursor to start.
    pub fn reset(&mut self) {
        self.symbols.clear();
        self.cursor_index = 0;
    }

    /// Current index of the cursor
    #[must_use]
    pub fn index(&self) -> usize {
        self.cursor_index
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// Returns the content typed by the user as a String.
    #[must_use]
    pub fn string(&self) -> String {
        self.symbols.join("")
    }

    /// Returns the index of the first displayed symbol on screen.
    /// It's the position of the left window of displayed symbols.
    ///
    /// If the input is short enough to be displayed completely, it's 0.
    /// If a text is too long to be displayed completely in the available rect,
    /// we scroll the text and display a window around it.
    ///
    /// It's the index of the cursor + a gap - the available space, clamped to 0.
    /// For example :
    /// input has 10 chars (self.len() = 10) like "abcdefghij"
    /// index is 6 : "abcdef|ghij" where | represents the cursor which doesn't take space.
    /// available space is 5, we can display at most 5 symbols.
    /// RIGHT_GAP is 4 - 4 chars should always be displayed at the right of the screen after the cursor, if possible.
    /// Then left index si 6 + 4 - 5 = 5
    /// We'll see abcd[ef|ghi]j, where [ ] represents the displayed text.
    #[inline]
    fn left_index(&self, available_space: usize) -> usize {
        if self.input_is_short_enough(available_space) {
            0
        } else {
            (self.index() + Self::RIGHT_GAP).saturating_sub(available_space)
        }
    }

    #[inline]
    fn input_is_short_enough(&self, available_space: usize) -> bool {
        self.len() <= available_space
    }

    /// Index on screen of the cursor.
    /// It's the index minus the left index considering the available space.
    #[inline]
    pub fn display_index(&self, available_space: usize) -> usize {
        self.index()
            .saturating_sub(self.left_index(available_space))
    }

    /// Returns the displayable input as a string accounting for available space.
    /// If the text is short enough to be displayed completely it's the string it self.
    pub fn scrolled_string(&self, available_space: usize) -> String {
        if self.input_is_short_enough(available_space) {
            self.string()
        } else {
            self.symbols
                .iter()
                .skip(self.left_index(available_space))
                .take(available_space)
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join("")
        }
    }

    /// Returns a string of * for every char typed.
    #[must_use]
    pub fn password(&self) -> String {
        "*".repeat(self.len())
    }

    /// Insert an utf-8 char into the input at cursor index.
    pub fn insert(&mut self, c: char) {
        self.symbols.insert(self.cursor_index, String::from(c));
        self.cursor_index += 1;
    }

    /// Insert a pasted string at current position.
    pub fn insert_string(&mut self, pasted: &str) {
        UnicodeSegmentation::graphemes(pasted, true)
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| (*s).to_string())
            .for_each(|s| {
                self.symbols.insert(self.cursor_index, s);
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
        if self.cursor_index > 0 && !self.symbols.is_empty() {
            self.symbols.remove(self.cursor_index - 1);
            self.cursor_index -= 1;
        }
    }

    /// Delete all chars right to the cursor
    pub fn delete_chars_right(&mut self) {
        self.symbols = self
            .symbols
            .iter()
            .take(self.cursor_index)
            .map(std::string::ToString::to_string)
            .collect();
    }

    pub fn delete_line(&mut self) {
        self.symbols = vec![];
        self.cursor_index = 0;
    }

    /// Deletes left symbols until a word is reached or the start of the line
    /// A word is delimited by a "separator"
    /// \t/\\.,;!?%_-+*(){}[]
    pub fn delete_left(&mut self) {
        while self.cursor_index > 0 {
            self.symbols.remove(self.cursor_index.saturating_sub(1));
            self.cursor_index -= 1;
            if self.is_empty() || is_separator(&self.symbols[self.cursor_index.saturating_sub(1)]) {
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
        self.symbols = UnicodeSegmentation::graphemes(content, true)
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        self.cursor_end();
    }

    /// Move the cursor to the previous "word".
    /// A word is delimited by a "separator"
    /// \t/\\.,;!?%_-+*(){}[]
    pub fn next_word(&mut self) {
        while self.cursor_index < self.symbols.len() {
            self.cursor_index += 1;
            if self.cursor_index == self.symbols.len()
                || is_separator(&self.symbols[self.cursor_index])
            {
                break;
            }
        }
    }

    /// Move the cursor to the previous "word".
    /// A word is delimited by a "separator"
    /// \t/\\.,;!?%_-+*(){}[]
    pub fn previous_word(&mut self) {
        while self.cursor_index > 0 {
            self.cursor_index -= 1;
            if self.cursor_index == 0
                || is_separator(&self.symbols[self.cursor_index.saturating_sub(1)])
            {
                break;
            }
        }
    }
}

#[rustfmt::skip]
#[inline]
fn is_separator(word: &str) -> bool {
    matches!(word, " " | "\t" | "/" | "\\" | "." | "," | ";" | "!" | "?" | "%" | "_" | "-" | "+" | "*" | "(" | ")" | "{" | "}" | "[" | "]") 
}
