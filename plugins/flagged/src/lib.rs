use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;

use plugin_api::{Askable, PluginInfo, PluginType, Updatable};

static mut PLUGIN_INFO: Option<PluginInfo> = None;

static mut FLAGGED_PATHS: Option<Vec<String>> = None;

#[no_mangle]
pub unsafe extern "C" fn plugin_entry() -> *mut PluginInfo {
    let plugin = PluginInfo {
        draw,
        on_event,
        ask,
        send,
        host_state_update,
        quit,
    };
    PLUGIN_INFO = Some(plugin);
    PLUGIN_INFO.as_mut().unwrap()
}

extern "C" fn ask() -> Vec<Askable> {
    vec![Askable::Flagged]
}

extern "C" fn send(data: Vec<PluginType>) {
    let mut paths = vec![];
    data.iter().for_each(|pp| {
        if let PluginType::VecString(flagged) = pp {
            paths = flagged.to_owned();
        }
    });
    unsafe {
        FLAGGED_PATHS = Some(paths);
    }
}

extern "C" fn draw(frame: &mut Frame, area: &Rect) {
    if let Some(paths) = unsafe { FLAGGED_PATHS.clone() } {
        draw_files(paths, frame, area);
    }
}

fn draw_files(paths: Vec<String>, frame: &mut Frame, area: &Rect) {
    paths.iter().enumerate().for_each(|(index, path)| {
        let sub_area = Rect {
            x: area.x + 1,
            y: area.y + 1 + index as u16,
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(1),
        };
        let line = Line::raw(path).yellow();
        line.render(sub_area, frame.buffer_mut());
    })
}

extern "C" fn host_state_update() -> Vec<Updatable> {
    vec![Updatable::Flagged(vec![])]
}

extern "C" fn quit() {
    unsafe {
        FLAGGED_PATHS = None;
    }
}

extern "C" fn on_event(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('p'))
}
