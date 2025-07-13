use crossterm::event::KeyEvent;
use ratatui::prelude::*;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// What kind of plugin is that ?
    pub kind: PluginKind,
    /// Draws the plugin on terminal
    /// Should be called by the displayer.
    pub draw: extern "C" fn(&mut Frame<'_>, &Rect),
    /// KeyEvent sent to the plugin. Returns `true` if the plugin consumed the event.
    pub on_event: extern "C" fn(KeyEvent) -> bool,
    // TODO! use pointer & size
    /// Asks the plugin about the host information it needs.
    /// It should be followed by a `send` call, made by the host.
    pub ask: extern "C" fn() -> Vec<Askable>,
    /// Send to the plugin the data it needs.
    pub send: extern "C" fn(Vec<PluginType>),
    /// Asks the plugin what host state should be updated.
    /// It should be called by the host after sending its data.
    pub host_state_update: extern "C" fn(FMContext) -> PluginContent,
    /// Quit the plugin, used to drop unused information and release memory.
    pub quit: extern "C" fn(),
}

pub type PluginEntryFn = unsafe extern "C" fn() -> *mut PluginInfo;

/// Different kinds of plugins
#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub enum PluginKind {
    Menu(MenuKind),
    Display,
    Previewer(Extensions),
}

/// Menu plugins
#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub enum MenuKind {
    Navigate,
    InputSimple,
    InputCompleted,
    NeedConfirmation,
}

/// Previewer plugins
#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Extensions {
    /// A string slice of extensions separated by a space
    /// like this : "pdf png jpg jpeg"
    /// Files with those extensions will previewed
    /// using the plugin instead of the default previewer.
    pub extensions: &'static str,
}

#[repr(C)]
#[derive(Clone, Debug, Default)]
pub enum PluginType {
    #[default]
    Empty,
    Bool(bool),
    Int(isize),
    UInt(usize),
    String(String),
    VecString(Vec<String>),
}

/// The different information the plugin can ask about.
/// Host should answer with corresponding data.
#[repr(C)]
#[derive(Clone, Debug, Copy)]
#[non_exhaustive]
pub enum Askable {
    /// List of flagged files
    Flagged,
    /// DisplayMode of this current tab
    DisplayMode,
    /// MenuMode of this current tab
    MenuMode,
    /// Current path in directory or tree
    CurrentPath,
    /// Current selection in directory or tree
    CurrentSelection,
}

#[repr(C)]
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub enum Updatable {
    #[default]
    Nothing,
    /// Jump to the given path
    Jump(String),
    /// Replace flagged files with this collection
    Flagged(Vec<String>),
    /// Change display mode
    DisplayMode(DisplayMode),
    /// Change menu mode
    MenuMode(String),
}

#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub enum DisplayMode {
    #[default]
    Directory,
    Tree,
    Preview,
    Fuzzy,
}

#[repr(C)]
#[derive(Clone, Debug, Default)]
pub struct PluginContent {
    pub content: Vec<String>,
    pub index: usize,
    pub updatables: Vec<Updatable>,
}

impl PluginContent {
    /// Reset all attributes to their default values:
    /// Content and updatable are cleared
    pub fn reset(&mut self) {
        self.content.clear();
        self.index = 0;
        self.updatables.clear();
    }

    /// True if content is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Number of element in content
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Select next element in content
    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.content.len()
    }

    /// Select previous element in content
    pub fn prev(&mut self) {
        if self.index > 0 {
            self.index -= 1
        } else {
            self.index = self.content.len() - 1
        }
    }

    /// Current index of selected element.
    /// 0 if the content is empty (I know, could be an option)
    pub fn index(&self) -> usize {
        self.index
    }

    /// set the index to the value if possible
    pub fn set_index(&mut self, index: usize) {
        if index < self.content.len() {
            self.index = index
        }
    }

    /// true if the selected element is the last of content
    pub fn selected_is_last(&self) -> bool {
        self.index + 1 == self.content.len()
    }
}

mod context;
pub use context::{FMContext, Focus, Order, SortBy, SortKind, StatusContext, TabContext};
