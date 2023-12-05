use crate::event::ActionMap;
use crate::impl_selectable_content;

const CONTEXT: [(&str, ActionMap); 9] = [
    ("Open", ActionMap::OpenFile),
    ("Open with", ActionMap::Exec),
    ("Flag", ActionMap::ToggleFlag),
    ("Rename", ActionMap::Rename),
    ("Delete", ActionMap::Delete),
    ("Trash", ActionMap::TrashMoveFile),
    ("Chmod", ActionMap::Chmod),
    ("New File", ActionMap::NewFile),
    ("New Directory", ActionMap::NewDir),
];

pub struct ContextMenu {
    pub content: Vec<&'static str>,
    index: usize,
    actions: Vec<&'static ActionMap>,
}

impl ContextMenu {
    pub fn default() -> Self {
        Self {
            index: 0,
            content: CONTEXT.iter().map(|(s, _)| *s).collect(),
            actions: CONTEXT.iter().map(|(_, a)| a).collect(),
        }
    }

    pub fn matcher(&self) -> &ActionMap {
        self.actions[self.index]
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }
}

type StaticStr = &'static str;

impl_selectable_content!(StaticStr, ContextMenu);
