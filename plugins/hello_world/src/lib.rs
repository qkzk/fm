use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

use plugin_api::PluginInfo;

static mut PLUGIN_INFO: Option<PluginInfo> = None;

#[no_mangle]
pub unsafe extern "C" fn plugin_entry() -> *mut PluginInfo {
    let state = HashMap::new();
    let plugin = PluginInfo {
        state,
        draw,
        on_event,
        update,
    };
    PLUGIN_INFO = Some(plugin);
    PLUGIN_INFO.as_mut().unwrap()
}

extern "C" fn draw(state: &HashMap<String, String>, frame: &mut Frame, area: &Rect) {
    let block = Block::default().red().title("Plugin").borders(Borders::ALL);
    block.render(*area, frame.buffer_mut());
    if let Some(val) = state.get("bla") {
        let sub_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(1),
        };
        let line = Line::raw(format!("bla -> {val}"));
        line.render(sub_area, frame.buffer_mut());
        // frame.render_widget(line, sub_area);
    }
}

extern "C" fn on_event(state: &mut HashMap<String, String>, key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('p'))
}

#[no_mangle]
extern "C" fn echo(data: u8) -> u8 {
    data
}

extern "C" fn update(state: &mut HashMap<String, String>, key: String, val: String) {
    state.insert(key, val);
}
