use std::borrow::Cow;
use std::cmp::min;

use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::{color_to_style, Canvas};
use crate::modes::{Content, ContentWindow};

/// Iter over the content, returning a triplet of `(index, line, attr)`.
#[macro_export]
macro_rules! colored_skip_take {
    ($t:ident, $u:ident) => {
        std::iter::zip(
            $t.iter().enumerate(),
            Gradient::new(
                ColorG::from_ratatui(
                    MENU_STYLES
                        .get()
                        .expect("Menu colors should be set")
                        .first
                        .fg
                        .unwrap_or(Color::Rgb(0, 0, 0)),
                )
                .unwrap_or_default(),
                ColorG::from_ratatui(
                    MENU_STYLES
                        .get()
                        .expect("Menu colors should be set")
                        .palette_3
                        .fg
                        .unwrap_or(Color::Rgb(0, 0, 0)),
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
    fn draw_menu(&self, f: &mut Frame, rect: &Rect, window: &ContentWindow)
    where
        Self: Content<T>,
    {
        let content = self.content();
        let mut spans = vec![];
        for (row, line, style) in colored_skip_take!(content, window) {
            let style = self.style(row, &style);
            // TODO: why ??????????????????????????????????????????????
            // let mut style = Style::default().fg(style.fg.unwrap_or(Color::Rgb(0, 0, 0)));
            // if let Some(modifier) = &style.add_modifier {
            //     style = style.add_modifier(modifier);
            // }
            let text = Line::from(Span::styled(line.cow_str(), style));
            spans.push(text);
            rect.print_with_style(
                f,
                (row + ContentWindow::WINDOW_MARGIN_TOP + 1 - window.top) as u16,
                4,
                &line.cow_str(),
                self.style(row, &style),
            );
            // Ok(Paragraph::new(spans).block(Block::default()));
        }
    }
}
