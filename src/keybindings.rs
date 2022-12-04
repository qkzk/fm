use std::collections::HashMap;

use log::info;

use crate::event_char::EventChar;

#[derive(Clone, Debug)]
pub struct Keybindings {
    pub binds: HashMap<char, EventChar>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Keybindings {
    const u8: ASCII_FIRST_PRINTABLE = 32;
    const u8: ASCII_LAST_PRINTABLE = 127;

    pub fn get(&self, key: &char) -> Option<&EventChar> {
        self.binds.get(key)
    }

    pub fn new() -> Self {
        let binds = HashMap::from([
            ('a', EventChar::ToggleHidden),
            ('c', EventChar::CopyPaste),
            ('p', EventChar::CutPaste),
            ('d', EventChar::NewDir),
            ('n', EventChar::NewFile),
            ('m', EventChar::Chmod),
            ('e', EventChar::Exec),
            ('g', EventChar::Goto),
            ('r', EventChar::Rename),
            ('u', EventChar::ClearFlags),
            (' ', EventChar::ToggleFlag),
            ('s', EventChar::Shell),
            ('x', EventChar::DeleteFile),
            ('o', EventChar::OpenFile),
            ('h', EventChar::Help),
            ('/', EventChar::Search),
            ('w', EventChar::RegexMatch),
            ('q', EventChar::Quit),
            ('*', EventChar::FlagAll),
            ('v', EventChar::ReverseFlags),
            ('j', EventChar::Jump),
            ('H', EventChar::History),
            ('i', EventChar::NvimFilepicker),
            ('O', EventChar::Sort),
            ('l', EventChar::Symlink),
            ('P', EventChar::Preview),
            ('G', EventChar::Shortcut),
            ('B', EventChar::Bulkrename),
            ('M', EventChar::MarksNew),
            ('\'', EventChar::MarksJump),
            ('F', EventChar::Filter),
            ('-', EventChar::Back),
            ('~', EventChar::Home),
        ]);
        Self { binds }
    }

    pub fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        for i in Self::ASCII_FIRST_PRINTABLE..=Self::ASCII_LAST_PRINTABLE {
            let key = i as char;
            let strng = key.to_string();
            if let Some(event_string) = yaml[strng].as_str().map(|s| s.to_string()) {
                info!("config: {} - {} - {:?}", i, key, event_string);
                self.binds.insert(key, EventChar::from(&event_string));
            }
        }
    }

    pub fn to_hashmap(&self) -> HashMap<String, String> {
        let mut reverse = HashMap::new();
        for (k, v) in self.binds.clone().into_iter() {
            let _ = reverse.insert(v.into(), k.into());
        }
        reverse
    }
}
