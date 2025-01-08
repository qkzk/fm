use std::{cmp::min, iter::zip};

use anyhow::Result;
use serde_yml::Mapping;

use crate::app::Status;
use crate::common::{is_in_path, TUIS_PATH};
use crate::config::{ColorG, Gradient, MENU_STYLES};
use crate::io::{color_to_style, CowStr, DrawMenu, External};
use crate::modes::{ContentWindow, Execute, TerminalApplications};
use crate::{colored_skip_take, impl_content, impl_selectable, log_info};
use ratatui::{
    layout::{Offset, Rect},
    prelude::Widget,
    style::Color,
    text::Line,
    widgets::Paragraph,
    Frame,
};

/// Directly open a a TUI application
/// The TUI application shares the same window as fm.
/// If the user picked "shell", we use the environment variable `$SHELL` or `bash` if it's not set.
pub fn open_tui_program(program: &str) -> Result<()> {
    if program == "shell" {
        External::open_shell_in_window()
    } else if is_in_path(program) {
        log_info!("Tui menu execute {program}");
        External::open_command_in_window(&[program])
    } else {
        log_info!("Tui menu program {program} isn't in path");
        Ok(())
    }
}

impl Execute<()> for String {
    fn execute(&self, _status: &Status) -> Result<()> {
        open_tui_program(self)
    }
}

/// Tui applications which requires a new terminal for interaction.
#[derive(Clone)]
pub struct TuiApplications {
    pub content: Vec<String>,
    index: usize,
}

impl TuiApplications {
    pub fn setup(&mut self) {
        self.update_from_config(TUIS_PATH);
    }

    pub fn is_not_set(&self) -> bool {
        self.content.len() == 1
    }
}

impl Default for TuiApplications {
    fn default() -> Self {
        let index = 0;
        let content = vec!["shell".to_owned()];
        Self { content, index }
    }
}

impl TerminalApplications<String, ()> for TuiApplications {
    fn parse_yaml(&mut self, yaml: &Mapping) {
        for (key, _) in yaml {
            let Some(command) = key.as_str() else {
                continue;
            };
            if is_in_path(command) {
                self.content.push(command.to_owned());
            }
        }
    }
}

impl_selectable!(TuiApplications);
impl_content!(String, TuiApplications);

impl DrawMenu<String> for TuiApplications {
    fn draw_menu(&self, f: &mut Frame, rect: &Rect, window: &ContentWindow)
    where
        Self: Content<String>,
    {
        let mut p_rect = rect.offset(Offset { x: 2, y: 3 }).intersection(*rect);
        p_rect.height = p_rect.height.saturating_sub(2);
        let content = self.content();
        let lines: Vec<_> = zip(
            ('a'..='z').cycle().skip(window.top),
            colored_skip_take!(content, window),
        )
        .filter(|(_, (index, _, _))| {
            (*index) as u16 + ContentWindow::WINDOW_MARGIN_TOP_U16 + 1 - window.top as u16 + 2
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
