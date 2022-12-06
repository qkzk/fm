use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use crate::action_map::ActionMap;
use crate::fm_error::FmResult;

#[derive(Clone, Debug)]
pub struct Keybindings {
    pub binds: HashMap<char, ActionMap>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Keybindings {
    const ASCII_FIRST_PRINTABLE: u8 = 32;
    const ASCII_LAST_PRINTABLE: u8 = 127;

    pub fn get(&self, key: &char) -> Option<&ActionMap> {
        self.binds.get(key)
    }

    pub fn new() -> Self {
        let binds = HashMap::from([
            ('a', ActionMap::ToggleHidden),
            ('c', ActionMap::CopyPaste),
            ('p', ActionMap::CutPaste),
            ('d', ActionMap::NewDir),
            ('n', ActionMap::NewFile),
            ('m', ActionMap::Chmod),
            ('e', ActionMap::Exec),
            ('g', ActionMap::Goto),
            ('r', ActionMap::Rename),
            ('u', ActionMap::ClearFlags),
            (' ', ActionMap::ToggleFlag),
            ('s', ActionMap::Shell),
            ('x', ActionMap::DeleteFile),
            ('o', ActionMap::OpenFile),
            ('h', ActionMap::Help),
            ('/', ActionMap::Search),
            ('w', ActionMap::RegexMatch),
            ('q', ActionMap::Quit),
            ('*', ActionMap::FlagAll),
            ('v', ActionMap::ReverseFlags),
            ('j', ActionMap::Jump),
            ('H', ActionMap::History),
            ('i', ActionMap::NvimFilepicker),
            ('O', ActionMap::Sort),
            ('l', ActionMap::Symlink),
            ('P', ActionMap::Preview),
            ('G', ActionMap::Shortcut),
            ('B', ActionMap::Bulkrename),
            ('M', ActionMap::MarksNew),
            ('\'', ActionMap::MarksJump),
            ('F', ActionMap::Filter),
            ('-', ActionMap::Back),
            ('~', ActionMap::Home),
        ]);
        Self { binds }
    }

    pub fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) -> FmResult<()> {
        for i in Self::ASCII_FIRST_PRINTABLE..=Self::ASCII_LAST_PRINTABLE {
            let key = i as char;
            let string = key.to_string();
            if let Some(event_string) = yaml[string].as_str().map(|event| event.to_string()) {
                self.binds.insert(key, ActionMap::from_str(&event_string)?);
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
