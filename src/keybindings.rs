use std::collections::HashMap;

use crate::event_char::EventChar;

pub struct KeyBinds {
    pub binds: HashMap<char, EventChar>,
}

impl KeyBinds {
    pub fn new(binds: HashMap<char, EventChar>) -> Self {
        Self { binds }
    }

    pub fn update(&mut self, key: char, event: EventChar) {
        self.binds.insert(key, event);
    }

    pub fn get(&self, key: char) -> Option<&EventChar> {
        self.binds.get(&key)
    }
}
