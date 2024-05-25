use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use tuikit::prelude::{from_keyname, Key};

use crate::common::CONFIG_PATH;
use crate::event::ActionMap;
use crate::log_info;

/// Holds an hashmap between keys and actions.
#[derive(Clone, Debug)]
pub struct Bindings {
    /// An HashMap of key & Actions.
    /// Every binded key is linked to its corresponding action
    pub binds: HashMap<Key, ActionMap>,
    /// Remember every key binded to a custom action
    pub custom: Option<Vec<String>>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Bindings {
    pub fn new() -> Self {
        let binds = HashMap::from([
            (Key::ESC, ActionMap::ResetMode),
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
            (Key::Char('`'), ActionMap::GoRoot),
            (Key::Char('!'), ActionMap::ShellCommand),
            (Key::Char('@'), ActionMap::GoStart),
            (Key::Char(':'), ActionMap::Action),
            (Key::Char('6'), ActionMap::History),
            (Key::Char('C'), ActionMap::Compress),
            (Key::Char('E'), ActionMap::ToggleDisplayFull),
            (Key::Char('G'), ActionMap::End),
            (Key::Char('F'), ActionMap::DisplayFlagged),
            (Key::Char('H'), ActionMap::FuzzyFindHelp),
            (Key::Char('J'), ActionMap::PageDown),
            (Key::Char('K'), ActionMap::PageUp),
            (Key::Char('I'), ActionMap::NvimSetAddress),
            (Key::Char('L'), ActionMap::Symlink),
            (Key::Char('M'), ActionMap::MarksNew),
            (Key::Char('O'), ActionMap::Sort),
            (Key::Char('P'), ActionMap::Preview),
            (Key::Char('X'), ActionMap::TrashMoveFile),
            (Key::Char('a'), ActionMap::ToggleHidden),
            (Key::Char('c'), ActionMap::CopyPaste),
            (Key::Char('d'), ActionMap::NewDir),
            (Key::Char('e'), ActionMap::Exec),
            (Key::Char('f'), ActionMap::SearchNext),
            (Key::Char('g'), ActionMap::KeyHome),
            (Key::Char('k'), ActionMap::MoveUp),
            (Key::Char('j'), ActionMap::MoveDown),
            (Key::Char('h'), ActionMap::MoveLeft),
            (Key::Char('l'), ActionMap::MoveRight),
            (Key::Char('i'), ActionMap::NvimFilepicker),
            (Key::Char('n'), ActionMap::NewFile),
            (Key::Char('o'), ActionMap::OpenFile),
            (Key::Char('m'), ActionMap::CutPaste),
            (Key::Char('q'), ActionMap::Quit),
            (Key::Char('r'), ActionMap::Rename),
            (Key::Char('s'), ActionMap::Shell),
            (Key::Char('t'), ActionMap::Tree),
            (Key::Char('u'), ActionMap::ClearFlags),
            (Key::Char('v'), ActionMap::ReverseFlags),
            (Key::Char('w'), ActionMap::RegexMatch),
            (Key::Char('x'), ActionMap::Delete),
            (Key::Char('z'), ActionMap::TreeFold),
            (Key::Char('Z'), ActionMap::TreeUnFoldAll),
            (Key::Alt('b'), ActionMap::Bulk),
            (Key::Alt('c'), ActionMap::OpenConfig),
            (Key::Alt('d'), ActionMap::ToggleDualPane),
            (Key::Alt('e'), ActionMap::EncryptedDrive),
            (Key::Alt('f'), ActionMap::Filter),
            (Key::Alt('g'), ActionMap::Cd),
            (Key::Alt('h'), ActionMap::Help),
            (Key::Alt('i'), ActionMap::CliMenu),
            (Key::Alt('l'), ActionMap::Log),
            (Key::Alt('o'), ActionMap::TrashOpen),
            (Key::Alt('r'), ActionMap::RemoteMount),
            (Key::Alt('s'), ActionMap::TuiMenu),
            (Key::Alt('R'), ActionMap::RemovableDevices),
            (Key::Alt('t'), ActionMap::Context),
            (Key::Alt('x'), ActionMap::TrashEmpty),
            (Key::Alt('m'), ActionMap::Chmod),
            (Key::Alt('p'), ActionMap::TogglePreviewSecond),
            (Key::Ctrl('c'), ActionMap::CopyFilename),
            (Key::Ctrl('d'), ActionMap::PageDown),
            (Key::Ctrl('f'), ActionMap::FuzzyFind),
            (Key::Ctrl('g'), ActionMap::Shortcut),
            (Key::Ctrl('s'), ActionMap::FuzzyFindLine),
            (Key::Ctrl('u'), ActionMap::PageUp),
            (Key::Ctrl('o'), ActionMap::OpenAll),
            (Key::Ctrl('p'), ActionMap::CopyFilepath),
            (Key::Ctrl('q'), ActionMap::ResetMode),
            (Key::Ctrl('r'), ActionMap::RefreshView),
            (Key::Ctrl('z'), ActionMap::TreeFoldAll),
            (Key::ShiftDown, ActionMap::NextThing),
            (Key::ShiftLeft, ActionMap::DeleteLine),
            (Key::ShiftUp, ActionMap::PreviousThing),
            (Key::CtrlUp, ActionMap::FocusGoUp),
            (Key::CtrlDown, ActionMap::FocusGoDown),
            (Key::CtrlRight, ActionMap::FocusGoRight),
            (Key::CtrlLeft, ActionMap::FocusGoLeft),
            (Key::Ctrl('h'), ActionMap::FocusGoLeft),
            (Key::Ctrl('j'), ActionMap::FocusGoDown),
            (Key::Ctrl('k'), ActionMap::FocusGoUp),
            (Key::Ctrl('l'), ActionMap::FocusGoRight),
            (Key::F(1), ActionMap::FuzzyFindHelp),
            (Key::F(2), ActionMap::Rename),
            (Key::F(3), ActionMap::Preview),
            (Key::F(4), ActionMap::OpenFile),
            (Key::F(5), ActionMap::CopyPaste),
            (Key::F(6), ActionMap::CutPaste),
            (Key::F(7), ActionMap::NewDir),
            (Key::F(8), ActionMap::Delete),
            (Key::F(9), ActionMap::NewFile),
            (Key::F(10), ActionMap::Quit),
            (Key::F(11), ActionMap::FlaggedToClipboard),
            (Key::F(12), ActionMap::FlaggedFromClipboard),
        ]);
        let custom = None;
        Self { binds, custom }
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
            .map(|(keybind, action)| (action.to_string(), format!("{keybind:?}")))
            .collect()
    }

    /// Update the binds from a config file.
    /// It may fail (and leave keybinding intact) if the file isn't formated properly.
    /// An unknown or poorly formated key will be ignored.
    pub fn update_normal(&mut self, yaml: &serde_yaml::value::Value) {
        let Some(mappings) = yaml.as_mapping() else {
            return;
        };
        for yaml_key in mappings.keys() {
            let Some(key_string) = yaml_key.as_str() else {
                log_info!("{CONFIG_PATH}: Keybinding {yaml_key:?} is unreadable");
                continue;
            };
            let Some(keymap) = from_keyname(key_string) else {
                log_info!("{CONFIG_PATH}: Keybinding {key_string} is unknown");
                continue;
            };
            let Some(action_str) = yaml[yaml_key].as_str() else {
                continue;
            };
            let Ok(action) = ActionMap::from_str(action_str) else {
                log_info!("{CONFIG_PATH}: Action {action_str} is unknown");
                continue;
            };
            self.binds.insert(keymap, action);
        }
    }

    pub fn update_custom(&mut self, yaml: &serde_yaml::value::Value) {
        let Some(mappings) = yaml.as_mapping() else {
            return;
        };
        let mut custom = vec![];
        for yaml_key in mappings.keys() {
            let Some(key_string) = yaml_key.as_str() else {
                log_info!("~/.config/fm/config.yaml: Keybinding {yaml_key:?} is unreadable");
                continue;
            };
            let Some(keymap) = from_keyname(key_string) else {
                log_info!("~/.config/fm/config.yaml: Keybinding {key_string} is unknown");
                continue;
            };
            let Some(custom_str) = yaml[yaml_key].as_str() else {
                continue;
            };
            let action = ActionMap::Custom(custom_str.to_owned());
            log_info!("custom bind {keymap:?}, {custom_str}");
            self.binds.insert(keymap, action.clone());
            custom.push(format!("{keymap:?}:        {custom_str}\n"));
        }
        self.custom = Some(custom);
    }
}
