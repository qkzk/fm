use std::cmp::{max, min};

/// How many rows are reserved for the header ?
pub const HEADER_ROWS: usize = 3;
/// How many rows are reserved for the footer ?
const FOOTER_ROWS: usize = 1;

/// Holds the information about the displayed window of lines.
/// When there's too much lines to display in one screen, we can scroll
/// and this struct is responsible for that.
/// Scrolling is done with `scroll_to`, `scroll_up_one`, `scroll_down_one`
/// methods.
#[derive(Debug, Clone)]
pub struct ContentWindow {
    /// The index of the first displayed file.
    pub top: usize,
    /// The index of the last displayed file + 1.
    pub bottom: usize,
    /// The number of displayble file in the current folder.
    pub len: usize,
    /// The height of the terminal.
    pub height: usize,
}

impl ContentWindow {
    /// Returns a new `ContentWindow` instance with values depending of
    /// number of files and height of the terminal screen.
    pub fn new(len: usize, terminal_height: usize) -> Self {
        let height = Self::nb_displayed_rows(terminal_height);
        ContentWindow {
            top: 0,
            bottom: min(len, height),
            len,
            height,
        }
    }

    /// The padding around the last displayed filename
    const WINDOW_PADDING: usize = 4;
    /// The space of the top element
    pub const WINDOW_MARGIN_TOP: usize = 2;
    /// How many rows are reserved at bottom

    /// Set the height of file window.
    pub fn set_height(&mut self, terminal_height: usize) {
        self.height = Self::nb_displayed_rows(terminal_height);
        self.bottom = min(self.len, self.height);
    }

    /// Move the window one line up if possible.
    /// Does nothing if the index can't be reached.
    pub fn scroll_up_one(&mut self, index: usize) {
        if (index < self.top + Self::WINDOW_PADDING || index > self.bottom) && self.top > 0 {
            self.top -= 1;
            self.bottom -= 1;
        }
        self.scroll_to(index)
    }

    /// Move the window one line down if possible.
    /// Does nothing if the index can't be reached.
    pub fn scroll_down_one(&mut self, index: usize) {
        if self.len < self.height {
            return;
        }
        if index < self.top || index > self.bottom - Self::WINDOW_PADDING {
            self.top += 1;
            self.bottom += 1;
        }

        self.scroll_to(index)
    }

    /// Reset the window to the first files of the current directory.
    pub fn reset(&mut self, len: usize) {
        self.len = len;
        self.top = 0;
        self.bottom = min(len, Self::nb_displayed_rows(self.height));
    }

    /// Scroll the window to this index if possible.
    /// Does nothing if the index can't be reached.
    pub fn scroll_to(&mut self, index: usize) {
        if index < self.top || index >= self.bottom {
            self.top = max(index, Self::WINDOW_PADDING) - Self::WINDOW_PADDING;
            self.bottom = self.top + min(self.len, Self::nb_displayed_rows(self.height));
        }
    }

    /// How many rows could be displayed with given height ?
    /// It's not the number of rows displayed since the content may
    /// not be long enough to fill the window.
    fn nb_displayed_rows(height: usize) -> usize {
        height - HEADER_ROWS - FOOTER_ROWS
    }
}
