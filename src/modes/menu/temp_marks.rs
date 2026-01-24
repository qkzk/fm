use std::{cmp::min, path::PathBuf};

use ratatui::{
    layout::Rect,
    style::Color,
    text::Line,
    widgets::{Paragraph, Widget},
    Frame,
};

use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::Offseted;
use crate::log_info;
use crate::modes::ContentWindow;
use crate::{colored_skip_take, impl_content, impl_selectable};

/// Temporary marks are saved in memory and reset when the application quit.
///
/// We save a fixed size vector of pathbufs.
/// The user can set a mark (default bind alt+") and jump to it (default bind ").
pub struct TempMarks {
    content: Vec<Option<PathBuf>>,
    pub index: usize,
}

impl Default for TempMarks {
    fn default() -> Self {
        let content = vec![None; Self::NB_TEMP_MARKS];
        let index = 0;
        Self { content, index }
    }
}

impl TempMarks {
    const NB_TEMP_MARKS: usize = 10;

    fn log_index_error(index: usize) {
        log_info!(
            "index {index} is too big for a temp mark. Should be between 0 and {NB_TEMP_MARKS} excluded",
            NB_TEMP_MARKS=Self::NB_TEMP_MARKS
        );
    }

    /// Set the mark at given index to the given path.
    pub fn set_mark(&mut self, index: usize, path: PathBuf) {
        if index >= Self::NB_TEMP_MARKS {
            Self::log_index_error(index);
            return;
        }
        self.content[index] = Some(path);
    }

    /// Reset the selected mark to `None`
    pub fn erase_current_mark(&mut self) {
        self.content[self.index] = None;
    }

    /// Get the indexed mark. `None` if the mark isn't set.
    pub fn get_mark(&self, index: usize) -> &Option<PathBuf> {
        if index >= Self::NB_TEMP_MARKS {
            Self::log_index_error(index);
            return &None;
        }
        &self.content[index]
    }

    /// Render the marks on the screen.
    /// Can't use the common trait nor the macro since `Option<PathBuf>` doesn't implement `CowStr`.
    pub fn draw_menu(&self, f: &mut Frame, rect: &Rect, window: &ContentWindow) {
        let mut p_rect = rect.offseted(2, 3);
        p_rect.height = p_rect.height.saturating_sub(2);
        let content = self.content();
        let lines: Vec<_> = colored_skip_take!(content, window)
            .filter(|(index, _, _)| {
                (*index) as u16 + ContentWindow::WINDOW_MARGIN_TOP_U16 + 1 - window.top as u16 + 2
                    <= rect.height
            })
            .map(|(index, opt_path, style)| {
                let content = if let Some(path) = opt_path {
                    format!("{index} {p}", p = path.display())
                } else {
                    format!("{index} ")
                };
                Line::styled(content, self.style(index, &style))
            })
            .collect();
        Paragraph::new(lines).render(p_rect, f.buffer_mut());
    }

    pub fn digit_for(&self, path: &std::path::Path) -> Option<usize> {
        for (index, opt_path) in self.content.iter().enumerate() {
            if let Some(p) = opt_path {
                if p == path {
                    return Some(index);
                }
            }
        }
        None
    }
}

type Opb = Option<PathBuf>;

impl_content!(TempMarks, Opb);
