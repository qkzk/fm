use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tuikit::prelude::*;

use crate::app::Status;
use crate::config::{ColorG, Gradient, MENU_ATTRS};
use crate::io::color_to_attr;
use crate::modes::ContentWindow;
use crate::{impl_content, impl_selectable};

macro_rules! enumerated_colored_iter {
    ($t:ident) => {
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
    };
}
/// Trait which should be implemented for every edit mode.
/// It says if leaving this mode should be followed with a reset of the display & file content,
/// and if we have to reset the edit mode.
pub trait Leave {
    fn leave(&mut self, status: &mut Status) -> Result<()>;
    /// Should the file content & window be refreshed when leaving this mode?
    fn must_refresh(&self) -> bool;
    /// Should the edit mode be reset to Nothing when leaving this mode ?
    fn must_reset_mode(&self) -> bool;
}

pub trait Draw {
    fn draw(&self, status: &Status, canvas: &mut dyn Canvas) -> Result<()>;
}

pub trait Navigation<T>: Content<T> + Leave + Draw {}

pub struct Navigable {
    pub window: ContentWindow,
    pub menu: Box<dyn Navigation<MenuItem>>,
}

impl Navigation<MenuItem> for History {}

impl Navigable {
    fn from_mode(mode: Navigate, window: ContentWindow) -> Self {
        match mode {
            Navigate::History => Self {
                window,
                menu: Box::new(History::default()),
            },
            Navigate::Shortcut => {
                todo!();
            }
            Navigate::Trash => {
                todo!();
            }
            Navigate::EncryptedDrive => {
                todo!();
            }
            Navigate::RemovableDevices => {
                todo!();
            }
            Navigate::Marks => {
                todo!();
            }
            Navigate::Compress => {
                todo!();
            }
            Navigate::TuiApplication => {
                todo!();
            }
            Navigate::CliApplication => {
                todo!();
            }
            Navigate::Context => {
                todo!();
            }
            Navigate::Cloud => {
                todo!();
            }
            Navigate::Picker => {
                todo!();
            }
            Navigate::Flagged => {
                todo!();
            }
        }
    }
}

#[derive(PartialEq, Clone)]
pub enum MenuItem {
    PB(PathBuf),
    ST(String),
    PAIR((String, String)),
    TRIPLET((String, String, String)),
}

// Next step: populate each Navigable with its kind
// regroup all implementations

/// A stack of visited paths.
/// We save the last folder and the selected file every time a `PatchContent` is updated.
/// We also ensure not to save the same pair multiple times.
#[derive(Default, Clone)]
struct History {
    pub content: Vec<MenuItem>,
    pub index: usize,
}

impl History {
    /// Add a new path and a selected file in the stack, without duplicates, and select the last
    /// one.
    pub fn push(&mut self, file: &Path) {
        if !self.content.contains(&MenuItem::PB(file.to_path_buf())) {
            self.content.push(MenuItem::PB(file.to_owned()));
            self.index = self.len() - 1;
        }
        // TODO! Else ... ?
    }

    /// Drop the last visited paths from the stack, after the selected one.
    /// Used to go back a few steps in time.
    pub fn drop_queue(&mut self) {
        if self.is_empty() {
            return;
        }
        let final_length = self.len() - self.index + 1;
        self.content.truncate(final_length);
        if self.is_empty() {
            self.index = 0;
        } else {
            self.index = self.len() - 1;
        }
    }

    /// True iff the last element of the stack has the same
    /// path as the one given.
    /// Doesn't check the associated file.
    /// false if the stack is empty.
    #[must_use]
    pub fn is_this_the_last(&self, path: &Path) -> bool {
        if self.is_empty() {
            return false;
        }
        let MenuItem::PB(last) = &self.content[self.len() - 1] else {
            return false;
        };
        last == path
    }
}
impl Leave for History {
    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    fn leave(&mut self, status: &mut Status) -> Result<()> {
        let Some(file) = self.selected() else {
            return Ok(());
        };
        let file = file.to_owned();
        let MenuItem::PB(file) = file else {
            return Ok(());
        };
        status.tabs[status.index].cd_to_file(&file)?;
        self.drop_queue();
        status.update_second_pane_for_preview()
    }
    /// Should the file content & window be refreshed when leaving this mode?
    fn must_refresh(&self) -> bool {
        false
    }
    /// Should the edit mode be reset to Nothing when leaving this mode ?
    fn must_reset_mode(&self) -> bool {
        false
    }
}

impl Draw for History {
    fn draw(&self, _: &Status, canvas: &mut dyn Canvas) -> Result<()> {
        let content = self.content();
        for (row, path, attr) in enumerated_colored_iter!(content) {
            let MenuItem::PB(path) = path else {
                continue;
            };
            let attr = self.attr(row, &attr);
            canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                path.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }
}

enum Navigate {
    /// Navigate back to a visited path
    History,
    /// Navigate to a predefined shortcut
    Shortcut,
    /// Manipulate trash files
    Trash,
    /// Manipulate an encrypted device
    EncryptedDrive,
    /// Removable devices
    RemovableDevices,
    /// Edit a mark or cd to it
    Marks,
    /// Pick a compression method
    Compress,
    /// Shell menu applications. Start a new shell with this application.
    TuiApplication,
    /// Cli info
    CliApplication,
    /// Context menu
    Context,
    /// Cloud menu
    Cloud,
    /// Picker menu
    Picker,
    /// Flagged files
    Flagged,
}

impl_selectable!(History);
impl_content!(MenuItem, History);
