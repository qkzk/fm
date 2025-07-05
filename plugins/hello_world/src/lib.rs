use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

use std::ffi::CString;

use plugin_api::{DrawContext, PluginInfo};

static mut PLUGIN_NAME: Option<CString> = None;
static mut PLUGIN_INFO: Option<PluginInfo> = None;

#[no_mangle]
pub unsafe extern "C" fn plugin_entry() -> *mut PluginInfo {
    let name = CString::new("PluginAfficheur").unwrap();
    PLUGIN_NAME = Some(name);
    let plugin = PluginInfo {
        name: PLUGIN_NAME.as_ref().unwrap().as_ptr() as *const u8,
        name_len: PLUGIN_NAME.as_ref().unwrap().to_bytes().len(),
        draw,
        on_event,
    };
    PLUGIN_INFO = Some(plugin);
    PLUGIN_INFO.as_mut().unwrap()
}

extern "C" fn draw(_ctx: &mut dyn DrawContext, frame: &mut Frame, area: Rect) {
    let block = Block::default().title("Plugin").borders(Borders::ALL);
    frame.render_widget(block, area);
}

extern "C" fn on_event(_ctx: &mut dyn DrawContext, key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('p'))
}
