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
    /// The height of the rect containing the elements.
    pub height: usize,
}

impl Default for ContentWindow {
    fn default() -> Self {
        Self::new(0, 80)
    }
}

impl ContentWindow {
    /// The padding around the last displayed filename
    pub const WINDOW_PADDING: usize = 4;
    /// The space of the top element as an u16 for convenience
    pub const WINDOW_MARGIN_TOP_U16: u16 = 2;
    /// The space for the bottom row
    pub const WINDOW_MARGIN_BOTTOM: usize = 1;
    /// How many rows are reserved for the header ?
    pub const HEADER_ROWS: usize = 3;
    /// How many rows are reserved for the footer ?
    const FOOTER_ROWS: usize = 1;

    /// How many rows could be displayed with given height ?
    /// It's not the number of rows displayed since the content may
    /// not be long enough to fill the window.
    fn nb_displayed_rows(rect_height: usize) -> usize {
        rect_height.saturating_sub(Self::HEADER_ROWS + Self::FOOTER_ROWS)
    }

    /// Default value for the bottom index.
    /// minimum of terminal height minus reserved rows and the length of the content.
    fn default_bottom(len: usize, used_height: usize) -> usize {
        min(len, used_height)
    }

    /// Returns a new `ContentWindow` instance with values depending of
    /// number of displayable elements and height of the terminal screen.
    pub fn new(len: usize, rect_height: usize) -> Self {
        let height = Self::nb_displayed_rows(rect_height);
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

    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.bottom = Self::default_bottom(len, self.height);
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
        if index < self.top || index + Self::WINDOW_PADDING > self.bottom {
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
            let height = max(self.bottom.saturating_sub(self.top), self.height);
            self.top = index.saturating_sub(Self::WINDOW_PADDING);
            self.bottom = self.top + height;
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

    pub fn preview_page_up(&mut self) {
        if self.top == 0 {
            return;
        }
        let skip = min(self.top, 30);
        self.bottom -= skip;
        self.top -= skip;
    }

    pub fn preview_page_down(&mut self, preview_len: usize) {
        if self.bottom < preview_len {
            let skip = min(preview_len - self.bottom, 30);
            self.bottom += skip;
            self.top += skip;
        }
    }
}
