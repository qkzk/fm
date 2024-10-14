use std::borrow::Cow;
use std::cmp::min;

use anyhow::Result;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::config::{ColorG, Gradient, MENU_ATTRS};
use crate::io::color_to_style;
use crate::modes::{Content, ContentWindow};

/// Iter over the content, returning a triplet of `(index, line, attr)`.
#[macro_export]
macro_rules! colored_skip_take {
    ($t:ident, $u:ident) => {
        std::iter::zip(
            $t.iter().enumerate(),
            Gradient::new(
                ColorG::from_tuikit(
                    MENU_ATTRS
                        .get()
                        .expect("Menu colors should be set")
                        .first
                        .fg,
                )
                .unwrap_or_default(),
                ColorG::from_tuikit(
                    MENU_ATTRS
                        .get()
                        .expect("Menu colors should be set")
                        .palette_3
                        .fg,
                )
                .unwrap_or_default(),
                $t.len(),
            )
            .gradient()
            .map(|color| color_to_style(color)),
        )
        .map(|((index, line), attr)| (index, line, attr))
        .skip($u.top)
        .take(min($u.bottom, $t.len()))
    };
}

/// Converts itself into a [`std::borrow::Cow<str>`].
/// It's used to call `print_with_attr` which requires an `&str`.
pub trait CowStr {
    fn cow_str(&self) -> Cow<str>;
}

impl CowStr for (char, std::path::PathBuf) {
    fn cow_str(&self) -> Cow<str> {
        format!("{c} {p}", c = self.0, p = self.1.display()).into()
    }
}

impl CowStr for std::path::PathBuf {
    fn cow_str(&self) -> Cow<str> {
        self.to_string_lossy()
    }
}

impl CowStr for String {
    fn cow_str(&self) -> Cow<str> {
        self.into()
    }
}

/// Trait used to display a scrollable content
/// Element are itered from the top to the bottom of the window index
/// and printed in the canvas.
/// Since the content kind is linked to a mode,
/// it prints the second line of the mode.
pub trait DrawMenu<T: CowStr> {
    fn draw_menu(&self, rect: &Rect, window: &ContentWindow) -> Result<Paragraph>
    where
        Self: Content<T>,
    {
        let content = self.content();
        let mut spans = vec![];
        for (row, line, attr) in colored_skip_take!(content, window) {
            let attr = self.style(row, &attr);
            let mut style = Style::default().fg(attr.fg);
            if let Some(modifier) = &attr.modifier {
                style = style.add_modifier(modifier);
            }
            let text = Line::from(Span::styled(&line.cow_str(), style));
            spans.push(text);
            // rect.print_with_attr(
            //     row + ContentWindow::WINDOW_MARGIN_TOP + 1 - window.top,
            //     4,
            //     &line.cow_str(),
            //     self.attr(row, &attr),
            // )?;
            Ok(Paragraph::new(spans).block(Block::default()));
        }
        Ok(())
    }
}
