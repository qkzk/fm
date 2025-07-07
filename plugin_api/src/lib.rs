use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::prelude::*;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PluginInfo {
    // Fonctions externes : draw et event
    pub state: HashMap<String, String>,
    pub draw: extern "C" fn(state: &HashMap<String, String>, &mut Frame<'_>, &Rect),
    pub on_event:
       
       
       
        extern "C" fn(state: &mut HashMap<String, String>, KeyEvent) -> bool,
    pub update: extern "C" fn(state: &mut HashMap<String, String>, String, String),
}


/// Signature de la fonction d'entrée
pub type PluginEntryFn = unsafe extern "C" fn() -> *mut PluginInfo;
