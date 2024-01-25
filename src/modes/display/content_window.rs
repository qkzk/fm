use std::cmp::{max, min};

/// Holds the information about the displayed window of lines.
/// When there's too much lines to display in one screen, we can scroll
/// and this struct is responsible for that.
/// Scrolling is done with `scroll_to`, `scroll_up_one`, `scroll_down_one`
/// methods.
#[derive(Debug, Clone)]
pub struct ContentWindow {
    /// The index of the first displayed element.
    pub top: usize,
    /// The index of the last displayed element + 1.
    pub bottom: usize,
    /// The number of displayble elements.
    pub len: usize,
    /// The height of the terminal.
    pub height: usize,
}

impl ContentWindow {
    /// The padding around the last displayed filename
    const WINDOW_PADDING: usize = 4;
    /// The space of the top element
    pub const WINDOW_MARGIN_TOP: usize = 2;
    /// The space for the bottom row
    pub const WINDOW_MARGIN_BOTTOM: usize = 1;
    /// How many rows are reserved for the header ?
    pub const HEADER_ROWS: usize = 3;
    /// How many rows are reserved for the footer ?
    const FOOTER_ROWS: usize = 1;
    /// Footer and bottom padding
    const BOTTOM_ROWS: usize = 2;

    /// How many rows could be displayed with given height ?
    /// It's not the number of rows displayed since the content may
    /// not be long enough to fill the window.
    fn nb_displayed_rows(height: usize) -> usize {
        height - Self::HEADER_ROWS - Self::FOOTER_ROWS
    }

    /// Default value for the bottom index.
    /// minimum of terminal height minus reserved rows and the length of the content.
    fn default_bottom(len: usize, height: usize) -> usize {
        min(height.saturating_sub(Self::BOTTOM_ROWS), len)
    }

    /// Returns a new `ContentWindow` instance with values depending of
    /// number of displayable elements and height of the terminal screen.
    pub fn new(len: usize, terminal_height: usize) -> Self {
        let height = Self::nb_displayed_rows(terminal_height);
        let top = 0;
        let bottom = Self::default_bottom(len, height);
        ContentWindow {
            top,
            bottom,
            len,
            height,
        }
    }

    /// Set the height of file window.
    pub fn set_height(&mut self, terminal_height: usize) {
        self.height = Self::nb_displayed_rows(terminal_height);
        self.bottom = Self::default_bottom(self.len, self.height);
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

    /// Scroll the window to this index if possible.
    /// Does nothing if the index can't be reached.
    pub fn scroll_to(&mut self, index: usize) {
        if self.len < self.height {
            return;
        }
        if self.is_index_outside_window(index) {
            self.top = max(index, Self::WINDOW_PADDING) - Self::WINDOW_PADDING;
            self.bottom = (self.top + Self::default_bottom(self.height, self.len))
                .checked_sub(Self::BOTTOM_ROWS)
                .unwrap_or(2);
        }
    }

    /// Reset the window to the first item of the content.
    pub fn reset(&mut self, len: usize) {
        self.len = len;
        self.top = 0;
        self.bottom = Self::default_bottom(self.len, self.height);
    }

    /// True iff the index is outside the displayed window or
    /// too close from the border.
    /// User shouldn't be able to reach the last elements
    fn is_index_outside_window(&self, index: usize) -> bool {
        index < self.top || index >= self.bottom
    }

    pub fn is_row_in_header(row: u16) -> bool {
        row < Self::HEADER_ROWS as u16
    }
}
