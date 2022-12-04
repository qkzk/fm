use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use crate::event_char::EventChar;
use crate::fm_error::FmResult;

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
    const ASCII_FIRST_PRINTABLE: u8 = 32;
    const ASCII_LAST_PRINTABLE: u8 = 127;

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

    pub fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) -> FmResult<()> {
        for i in Self::ASCII_FIRST_PRINTABLE..=Self::ASCII_LAST_PRINTABLE {
            let key = i as char;
            let string = key.to_string();
            if let Some(event_string) = yaml[string].as_str().map(|event| event.to_string()) {
                self.binds.insert(key, EventChar::from_str(&event_string)?);
            }
        }
        Ok(())
    }

    pub fn to_hashmap(&self) -> HashMap<String, String> {
        self.binds
            .clone()
            .into_iter()
            .map(|(k, v)| (v.to_string(), k.into()))
            .collect()
    }
}
