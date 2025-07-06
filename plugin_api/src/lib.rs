use crossterm::event::KeyEvent;
use ratatui::prelude::*;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct PluginInfo {
    // Fonctions externes : draw et event
    pub draw: extern "C" fn(&mut dyn DrawContext, &mut Frame<'_>, Rect),
    pub on_event: extern "C" fn(&mut dyn DrawContext, KeyEvent) -> bool,
}

/// Trait que l'application implémente pour passer du contexte au plugin
/// Exemple : accès au système de logs ou de message
pub trait DrawContext {}

/// Signature de la fonction d'entrée
pub type PluginEntryFn = unsafe extern "C" fn() -> *mut PluginInfo;
