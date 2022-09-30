use std::cmp::{max, min};

const WINDOW_PADDING: usize = 4;
pub const WINDOW_MARGIN_TOP: usize = 1;

pub struct FilesWindow {
    pub top: usize,
    pub bottom: usize,
    pub len: usize,
    pub height: usize,
}

impl FilesWindow {
    pub fn new(len: usize, height: usize) -> Self {
        FilesWindow {
            top: 0,
            bottom: min(len, height - 3),
            len,
            height: height - 3,
        }
    }

    pub fn scroll_up_one(&mut self, index: usize) {
        if index < self.top + WINDOW_PADDING && self.top > 0 {
            self.top -= 1;
            self.bottom -= 1;
        }
    }

    pub fn scroll_down_one(&mut self, index: usize) {
        if self.len < self.height {
            return;
        }
        if index > self.bottom - WINDOW_PADDING && self.bottom < self.len - WINDOW_MARGIN_TOP {
            self.top += 1;
            self.bottom += 1;
        }
    }

    pub fn reset(&mut self, len: usize) {
        self.len = len;
        self.top = 0;
        self.bottom = min(len, self.height);
    }

    pub fn scroll_to(&mut self, index: usize) {
        if index < self.top || index > self.bottom {
            self.top = max(index, WINDOW_PADDING) - WINDOW_PADDING;
            self.bottom = self.top + min(self.len, self.height - 3);
        }
    }
}
