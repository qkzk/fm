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
        for i in 32_u8..=127_u8 {
            let c = i as char;
            let s = c.to_string();
            if let Some(v) = yaml[s].as_str().map(|s| s.to_string()) {
                info!("config: {} - {} - {:?}", i, c, v);
                self.binds.insert(c, EventChar::from(&v));
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
