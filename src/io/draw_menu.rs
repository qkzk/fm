use std::borrow::Cow;
use std::cmp::min;

use anyhow::Result;
use tuikit::prelude::Canvas;

use crate::config::{ColorG, Gradient, MENU_ATTRS};
use crate::io::color_to_attr;
use crate::modes::{Content, ContentWindow, SecondLine};

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
            .map(|color| color_to_attr(color)),
        )
        .map(|((index, line), attr)| (index, line, attr))
        .skip($u.top)
        .take(min($u.bottom, $t.len()))
    };
}

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

pub trait DrawMenu<U: CowStr> {
    fn draw_menu(
        &self,
        canvas: &mut dyn Canvas,
        window: &ContentWindow,
        mode: &dyn SecondLine,
    ) -> Result<()>
    where
        Self: Content<U>,
    {
        canvas.print(1, 2, mode.second_line())?;
        let content = self.content();
        for (row, line, attr) in colored_skip_take!(content, window) {
            canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP + 1 - window.top,
                4,
                &line.cow_str(),
                self.attr(row, &attr),
            )?;
        }
        Ok(())
    }
}