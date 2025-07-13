use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;

use plugin_api::{
    Askable, FMContext, MenuKind, PluginContent, PluginInfo, PluginKind, PluginType, Updatable,
};

static mut PLUGIN_INFO: Option<PluginInfo> = None;

static mut STATE: PluginStatus = PluginStatus::default();

#[no_mangle]
pub unsafe extern "C" fn plugin_entry() -> *mut PluginInfo {
    let plugin = PluginInfo {
        kind: PluginKind::Menu(MenuKind::Navigate),
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
        STATE.len = paths.len();
        STATE.flagged_path = paths;
    }
}

extern "C" fn draw(frame: &mut Frame, area: &Rect) {
    unsafe {
        draw_files(&STATE.flagged_path, frame, area);
    }
}

fn draw_files(paths: &[String], frame: &mut Frame, area: &Rect) {
    paths.iter().enumerate().for_each(|(index, path)| {
        let sub_area = Rect {
            x: area.x + 1,
            y: area.y + 1 + index as u16,
            width: area.width.saturating_sub(1),
            height: area.height.saturating_sub(1),
        };
        let mut line = Line::raw(path).yellow();
        if unsafe { index == STATE.index } {
            line = line.bold();
        }
        line.render(sub_area, frame.buffer_mut());
    })
}

extern "C" fn host_state_update(ctx: FMContext) -> PluginContent {
    unsafe {
        let plugin_content = STATE.to_plugin_content();
        STATE.reset_updatable();
        plugin_content
    }
}

extern "C" fn quit() {
    unsafe {
        STATE = PluginStatus::default();
    }
}

extern "C" fn on_event(key: KeyEvent) -> bool {
    match key {
        KeyEvent {
            code: KeyCode::Up,
            modifiers: _,
            kind: _,
            state: _,
        } => select_prev(),
        KeyEvent {
            code: KeyCode::Down,
            modifiers: _,
            kind: _,
            state: _,
        } => select_next(),
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: _,
            kind: _,
            state: _,
        } => jump(),
        _ => {
            return false;
        }
    };
    true
}

#[derive(Clone, Debug)]
struct PluginStatus {
    index: usize,
    len: usize,
    flagged_path: Vec<String>,
    updatable: Updatable,
}

impl PluginStatus {
    const fn default() -> Self {
        Self {
            index: 0,
            len: 0,
            flagged_path: vec![],
            updatable: Updatable::Nothing,
        }
    }

    fn select_next(&mut self) {
        self.index = (self.index + 1) % self.len;
    }

    fn select_prev(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }

    fn jump(&mut self) {
        if self.len > 0 {
            self.updatable = Updatable::Jump(self.flagged_path[self.index].to_owned())
        }
    }

    fn reset_updatable(&mut self) {
        self.updatable = Updatable::Nothing;
    }

    pub fn to_plugin_content(&self) -> PluginContent {
        PluginContent {
            index: self.index,
            content: self.flagged_path.clone(),
            updatables: vec![self.updatable.clone()],
        }
    }
}

fn select_next() {
    unsafe { STATE.select_next() }
}

fn select_prev() {
    unsafe { STATE.select_prev() }
}

fn jump() {
    unsafe { STATE.jump() }
}
