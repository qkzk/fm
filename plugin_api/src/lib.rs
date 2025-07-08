use crossterm::event::KeyEvent;
use ratatui::prelude::*;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub draw: extern "C" fn(&mut Frame<'_>, &Rect),
    pub on_event: extern "C" fn(KeyEvent) -> bool,
    // TODO! use pointer & size
    pub ask: extern "C" fn() -> Vec<Askable>,
    pub send: extern "C" fn(Vec<PluginType>),
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

#[repr(C)]
#[derive(Clone, Debug)]
pub enum Askable {
    Flagged,
    DisplayMode,
    MenuMode,
    CurrentPath,
    CurrentSelection,
}
