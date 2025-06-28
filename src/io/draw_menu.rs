use std::borrow::Cow;
use std::cmp::min;

use ratatui::{layout::Rect, prelude::Widget, style::Color, text::Line, widgets::Paragraph, Frame};

use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::{color_to_style, Offseted};
use crate::modes::{Content, ContentWindow};

/// Iter over the content, returning a triplet of `(index, line, style)`.
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
        .map(|((index, line), style)| (index, line, style))
        .skip($u.top)
        .take(min($u.len, $t.len()))
    };
}

/// Converts itself into a [`std::borrow::Cow<str>`].
/// It's used to call `print_with_style` which requires an `&str`.
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

impl CowStr for &str {
    fn cow_str(&self) -> Cow<str> {
        (*self).into()
    }
}

/// Trait used to display a scrollable content
/// Element are itered from the top to the bottom of the window index
/// and printed in the canvas.
/// Since the content kind is linked to a mode,
/// it doesn't print the second line of the mode.
pub trait DrawMenu<T: CowStr> {
    fn draw_menu(&self, f: &mut Frame, rect: &Rect, window: &ContentWindow)
    where
        Self: Content<T>,
    {
        let mut p_rect = rect.offseted(4, 3);
        p_rect.height = p_rect.height.saturating_sub(2);
        let content = self.content();
        let lines: Vec<_> = colored_skip_take!(content, window)
            .map(|(index, item, style)| Line::styled(item.cow_str(), self.style(index, &style)))
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }
}

#[macro_export]
macro_rules! impl_draw_menu_with_char {
    ($struct:ident, $field_type:ty) => {
        use std::{cmp::min, iter::zip};

        use ratatui::{
            layout::{Offset, Rect},
            prelude::Widget,
            style::Color,
            text::Line,
            widgets::Paragraph,
            Frame,
        };

        use $crate::colored_skip_take;
        use $crate::config::{ColorG, Gradient, MENU_STYLES};
        use $crate::io::{color_to_style, CowStr, DrawMenu};
        use $crate::modes::ContentWindow;

        impl DrawMenu<$field_type> for $struct {
            fn draw_menu(&self, f: &mut Frame, rect: &Rect, window: &ContentWindow)
            where
                Self: Content<$field_type>,
            {
                let mut p_rect = rect.offset(Offset { x: 2, y: 3 }).intersection(*rect);
                p_rect.height = p_rect.height.saturating_sub(2);
                let content = self.content();
                let lines: Vec<_> = zip(
                    ('a'..='z').cycle().skip(window.top),
                    colored_skip_take!(content, window),
                )
                .filter(|(_, (index, _, _))| {
                    ((*index) as u16 + ContentWindow::WINDOW_MARGIN_TOP_U16 + 1)
                        .saturating_sub(window.top as u16)
                        + 2
                        <= rect.height
                })
                .map(|(letter, (index, path, style))| {
                    Line::styled(
                        format!("{letter} {path}", path = path.cow_str()),
                        self.style(index, &style),
                    )
                })
                .collect();
                Paragraph::new(lines).render(p_rect, f.buffer_mut());
            }
        }
    };
}
