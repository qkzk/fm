use std::ops::{Bound, RangeBounds};

use tuikit::attr::Attr;
use tuikit::prelude::*;

use crate::io::Height;

#[derive(Default)]
pub struct Line {
    components: Vec<Component>,
    position: Position,
    offset: usize,
}

impl Widget for Line {}

impl Draw for Line {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let height = canvas.height().unwrap_or_default();
        let row = match self.position {
            Position::Top => 0,
            Position::Bottom => height - 1,
        };
        let mut col = self.offset;
        for component in &self.components {
            col += component.print(canvas, row, col)?;
        }
        Ok(())
    }
}

#[derive(Default)]
enum Position {
    #[default]
    Top,
    Bottom,
}

#[derive(Default)]
struct Component {
    text: String,
    attr: Attr,
    size: Size,
}

impl Component {
    fn content(&self) -> &str {
        match self.size {
            Size::Free => self.text.as_str(),
            Size::Fixed(size) => self.text.as_str().slice(..size),
        }
    }

    fn padding_right(&self) -> usize {
        match self.size {
            Size::Free => 0,
            Size::Fixed(size) => size.checked_sub(self.text.len()).unwrap_or_default(),
        }
    }

    fn print(&self, canvas: &mut dyn Canvas, row: usize, col: usize) -> Result<usize> {
        let col = canvas.print_with_attr(row, col, self.content(), self.attr)?;
        Ok(col + canvas.print(row, col, &" ".repeat(self.padding_right()))?)
    }
}

#[derive(Default)]
enum Size {
    #[default]
    Free,
    Fixed(usize),
}

trait StringUtils {
    fn substring(&self, start: usize, len: usize) -> &str;
    fn slice(&self, range: impl RangeBounds<usize>) -> &str;
}

impl StringUtils for str {
    fn substring(&self, start: usize, len: usize) -> &str {
        let mut char_pos = 0;
        let mut byte_start = 0;
        let mut it = self.chars();
        loop {
            if char_pos == start {
                break;
            }
            if let Some(c) = it.next() {
                char_pos += 1;
                byte_start += c.len_utf8();
            } else {
                break;
            }
        }
        char_pos = 0;
        let mut byte_end = byte_start;
        loop {
            if char_pos == len {
                break;
            }
            if let Some(c) = it.next() {
                char_pos += 1;
                byte_end += c.len_utf8();
            } else {
                break;
            }
        }
        &self[byte_start..byte_end]
    }
    fn slice(&self, range: impl RangeBounds<usize>) -> &str {
        let start = match range.start_bound() {
            Bound::Included(bound) | Bound::Excluded(bound) => *bound,
            Bound::Unbounded => 0,
        };
        let len = match range.end_bound() {
            Bound::Included(bound) => *bound + 1,
            Bound::Excluded(bound) => *bound,
            Bound::Unbounded => self.len(),
        } - start;
        self.substring(start, len)
    }
}
