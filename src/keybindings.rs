use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use tuikit::prelude::{from_keyname, Key};

use crate::action_map::ActionMap;
use crate::fm_error::FmResult;

/// Holds an hashmap between keys and actions.
#[derive(Clone, Debug)]
pub struct Bindings {
    /// An HashMap of key & Actions.
    /// Every binded key is linked to its corresponding action
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
            (Key::ESC, ActionMap::ModeNormal),
            (Key::Up, ActionMap::MoveUp),
            (Key::Down, ActionMap::MoveDown),
            (Key::Left, ActionMap::MoveLeft),
            (Key::Right, ActionMap::MoveRight),
            (Key::Backspace, ActionMap::Backspace),
            (Key::Delete, ActionMap::Delete),
            (Key::Home, ActionMap::KeyHome),
            (Key::End, ActionMap::End),
            (Key::PageDown, ActionMap::PageDown),
            (Key::PageUp, ActionMap::PageUp),
            (Key::Enter, ActionMap::Enter),
            (Key::Tab, ActionMap::Tab),
            (Key::BackTab, ActionMap::BackTab),
            (Key::Char(' '), ActionMap::ToggleFlag),
            (Key::Char('/'), ActionMap::Search),
            (Key::Char('*'), ActionMap::FlagAll),
            (Key::Char('\''), ActionMap::MarksJump),
            (Key::Char('-'), ActionMap::Back),
            (Key::Char('~'), ActionMap::Home),
            (Key::Char(':'), ActionMap::Command),
            (Key::Char('B'), ActionMap::Bulk),
            (Key::Char('C'), ActionMap::Compress),
            (Key::Char('D'), ActionMap::Diff),
            (Key::Char('E'), ActionMap::EncryptedDrive),
            (Key::Char('F'), ActionMap::Filter),
            (Key::Char('G'), ActionMap::Shortcut),
            (Key::Char('H'), ActionMap::History),
            (Key::Char('I'), ActionMap::NvimSetAddress),
            (Key::Char('M'), ActionMap::MarksNew),
            (Key::Char('O'), ActionMap::Sort),
            (Key::Char('P'), ActionMap::Preview),
            (Key::Char('T'), ActionMap::MediaInfo),
            (Key::Char('X'), ActionMap::TrashMoveFile),
            (Key::Char('W'), ActionMap::SetWallpaper),
            (Key::Char('a'), ActionMap::ToggleHidden),
            (Key::Char('c'), ActionMap::CopyPaste),
            (Key::Char('d'), ActionMap::NewDir),
            (Key::Char('e'), ActionMap::Exec),
            (Key::Char('f'), ActionMap::SearchNext),
            (Key::Char('g'), ActionMap::Goto),
            (Key::Char('h'), ActionMap::Help),
            (Key::Char('i'), ActionMap::NvimFilepicker),
            (Key::Char('j'), ActionMap::Jump),
            (Key::Char('l'), ActionMap::Symlink),
            (Key::Char('L'), ActionMap::Lazygit),
            (Key::Char('m'), ActionMap::Chmod),
            (Key::Char('n'), ActionMap::NewFile),
            (Key::Char('o'), ActionMap::OpenFile),
            (Key::Char('p'), ActionMap::CutPaste),
            (Key::Char('q'), ActionMap::Quit),
            (Key::Char('r'), ActionMap::Rename),
            (Key::Char('s'), ActionMap::Shell),
            (Key::Char('t'), ActionMap::Tree),
            (Key::Char('u'), ActionMap::ClearFlags),
            (Key::Char('v'), ActionMap::ReverseFlags),
            (Key::Char('w'), ActionMap::RegexMatch),
            (Key::Char('x'), ActionMap::DeleteFile),
            (Key::Char('z'), ActionMap::TreeFold),
            (Key::Char('Z'), ActionMap::TreeUnFoldAll),
            (Key::Alt('z'), ActionMap::TreeFoldAll),
            (Key::Alt('d'), ActionMap::DragNDrop),
            (Key::Alt('e'), ActionMap::ToggleDisplayFull),
            (Key::Alt('f'), ActionMap::ToggleDualPane),
            (Key::Alt('p'), ActionMap::TogglePreviewSecond),
            (Key::Alt('c'), ActionMap::OpenConfig),
            (Key::Alt('x'), ActionMap::TrashEmpty),
            (Key::Alt('o'), ActionMap::TrashOpen),
            (Key::Ctrl('c'), ActionMap::CopyFilename),
            (Key::Ctrl('d'), ActionMap::Delete),
            (Key::Ctrl('f'), ActionMap::FuzzyFind),
            (Key::Ctrl('s'), ActionMap::FuzzyFindLine),
            (Key::Ctrl('p'), ActionMap::CopyFilepath),
            (Key::Ctrl('q'), ActionMap::ModeNormal),
            (Key::Ctrl('r'), ActionMap::RefreshView),
            (Key::CtrlUp, ActionMap::MocpAddToPlayList),
            (Key::CtrlDown, ActionMap::MocpTogglePause),
            (Key::CtrlRight, ActionMap::MocpNext),
            (Key::CtrlLeft, ActionMap::MocpPrevious),
        ]);
        Self { binds }
    }

    /// Returns an Option of action. None if the key isn't binded.
    pub fn get(&self, key: &Key) -> Option<&ActionMap> {
        self.binds.get(key)
    }

    /// Reverse the hashmap of keys.
    /// Used to format the help string.
    pub fn keybind_reversed(&self) -> HashMap<String, String> {
        self.binds
            .iter()
            .map(|(k, v)| (v.to_string(), format!("{k:?}")))
            .collect()
    }

    /// Update the binds from a config file.
    /// It may fail (and leave keybinding intact) if the file isn't formated properly.
    /// An unknown or poorly formated key will be ignored.
    pub fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) -> FmResult<()> {
        for yaml_key in yaml.as_mapping().unwrap().keys() {
            let Some(key_string) = yaml_key.as_str() else { return Ok(()) };
            let Some(keymap) = from_keyname(key_string) else {return Ok(())};
            let Some(action_str) = yaml[yaml_key].as_str() else { return Ok(())};
            self.binds.insert(keymap, ActionMap::from_str(action_str)?);
        }
        Ok(())
    }
}
