use crossterm::event::KeyEvent;
use ratatui::prelude::*;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PluginInfo {
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
    pub host_state_update: extern "C" fn(FMContext) -> Vec<Updatable>,
    /// Quit the plugin, used to drop unused information and release memory.
    pub quit: extern "C" fn(),
}

pub type PluginEntryFn = unsafe extern "C" fn() -> *mut PluginInfo;

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
#[derive(Clone, Debug)]
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

mod context;
pub use context::{FMContext, Focus, Order, SortBy, SortKind, StatusContext, TabContext};
