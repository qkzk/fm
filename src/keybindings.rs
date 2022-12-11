use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use log::info;
use tuikit::prelude::{from_keyname, Key};

use crate::action_map::ActionMap;
use crate::fm_error::FmResult;

#[derive(Clone, Debug)]
pub struct Bindings {
    pub binds: HashMap<Key, ActionMap>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Bindings {
    fn new() -> Self {
        let binds = HashMap::from([
            (Key::Char('a'), ActionMap::ToggleHidden),
            (Key::Char('c'), ActionMap::CopyPaste),
            (Key::Char('p'), ActionMap::CutPaste),
            (Key::Char('d'), ActionMap::NewDir),
            (Key::Char('n'), ActionMap::NewFile),
            (Key::Char('m'), ActionMap::Chmod),
            (Key::Char('e'), ActionMap::Exec),
            (Key::Char('g'), ActionMap::Goto),
            (Key::Char('r'), ActionMap::Rename),
            (Key::Char('u'), ActionMap::ClearFlags),
            (Key::Char(' '), ActionMap::ToggleFlag),
            (Key::Char('s'), ActionMap::Shell),
            (Key::Char('x'), ActionMap::DeleteFile),
            (Key::Char('o'), ActionMap::OpenFile),
            (Key::Char('h'), ActionMap::Help),
            (Key::Char('/'), ActionMap::Search),
            (Key::Char('w'), ActionMap::RegexMatch),
            (Key::Char('q'), ActionMap::Quit),
            (Key::Char('*'), ActionMap::FlagAll),
            (Key::Char('v'), ActionMap::ReverseFlags),
            (Key::Char('j'), ActionMap::Jump),
            (Key::Char('H'), ActionMap::History),
            (Key::Char('i'), ActionMap::NvimFilepicker),
            (Key::Char('O'), ActionMap::Sort),
            (Key::Char('l'), ActionMap::Symlink),
            (Key::Char('P'), ActionMap::Preview),
            (Key::Char('G'), ActionMap::Shortcut),
            (Key::Char('B'), ActionMap::Bulkrename),
            (Key::Char('M'), ActionMap::MarksNew),
            (Key::Char('\\'), ActionMap::MarksJump),
            (Key::Char('F'), ActionMap::Filter),
            (Key::Char('-'), ActionMap::Back),
            (Key::Char('~'), ActionMap::Home),
            (Key::ESC, ActionMap::ModeNormal),
            (Key::Up, ActionMap::MoveUp),
            (Key::Down, ActionMap::MoveDown),
            (Key::Left, ActionMap::MoveLeft),
            (Key::Right, ActionMap::MoveRight),
            (Key::Backspace, ActionMap::Backspace),
            (Key::Ctrl('d'), ActionMap::Delete),
            (Key::Ctrl('q'), ActionMap::ModeNormal),
            (Key::Home, ActionMap::KeyHome),
            (Key::End, ActionMap::End),
            (Key::PageDown, ActionMap::PageDown),
            (Key::PageUp, ActionMap::PageUp),
            (Key::Enter, ActionMap::Enter),
            (Key::Tab, ActionMap::Tab),
            (Key::BackTab, ActionMap::BackTab),
            (Key::Ctrl('f'), ActionMap::FuzzyFind),
            (Key::Ctrl('c'), ActionMap::CopyFilename),
            (Key::Ctrl('p'), ActionMap::CopyFilepath),
            (Key::Ctrl('r'), ActionMap::RefreshView),
            (Key::Ctrl('x'), ActionMap::Decompress),
            (Key::Ctrl('e'), ActionMap::DisplayFull),
            (Key::Alt('d'), ActionMap::DragNDrop),
        ]);
        Self { binds }
    }

    pub fn get(&self, key: &Key) -> Option<&ActionMap> {
        self.binds.get(key)
    }

    pub fn keybind_reversed(&self) -> HashMap<String, String> {
        let b = self
            .binds
            .clone()
            .into_iter()
            .map(|(k, v)| (v.to_string(), format!("{:?}", k)))
            .collect();
        b
    }

    pub fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) -> FmResult<()> {
        for yaml_key in yaml.as_mapping().unwrap().keys() {
            if let Some(key_string) = yaml_key.as_str() {
                if let Some(keymap) = from_keyname(key_string) {
                    if let Some(action_str) = yaml[yaml_key].as_str() {
                        let action_map = ActionMap::from_str(action_str);
                        info!("update {:?}-{}", keymap, action_str);
                        self.binds.insert(keymap, ActionMap::from_str(action_str)?);
                    }
                }
            }
        }
        Ok(())
    }
}
