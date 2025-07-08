use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

use plugin_api::{Askable, PluginInfo, PluginType};

static mut PLUGIN_INFO: Option<PluginInfo> = None;
static mut SELECTED: Option<String> = None;

#[no_mangle]
pub unsafe extern "C" fn plugin_entry() -> *mut PluginInfo {
    let plugin = PluginInfo {
        draw,
        on_event,
        ask,
        send,
        quit,
    };
    PLUGIN_INFO = Some(plugin);
    PLUGIN_INFO.as_mut().unwrap()
}

extern "C" fn draw(frame: &mut Frame, area: &Rect) {
    let block = Block::default()
        .red()
        .title("Hello World")
        .borders(Borders::ALL);
    block.render(*area, frame.buffer_mut());
    if let Some(selected) = unsafe { SELECTED.clone() } {
        let line = Line::raw(selected).cyan();
        let mut sub_area = *area;
        sub_area.y += 4;
        sub_area.x += 2;
        line.render(sub_area, frame.buffer_mut());
    }
}

extern "C" fn ask() -> Vec<Askable> {
    vec![Askable::CurrentSelection]
}

extern "C" fn send(mut data: Vec<PluginType>) {
    if data.len() != 1 {
        return;
    }
    if let PluginType::String(selected) = std::mem::take(&mut data[0]) {
        unsafe {
            SELECTED = Some(selected);
        }
    }
}

extern "C" fn on_event(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('p'))
}

extern "C" fn quit() {
    unsafe {
        SELECTED = None;
    }
}
