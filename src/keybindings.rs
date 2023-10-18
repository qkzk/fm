use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use tuikit::prelude::{from_keyname, Key};

use crate::action_map::ActionMap;
use crate::constant_strings_paths::CONFIG_PATH;

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
    fn new() -> Self {
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
            (Key::Char(':'), ActionMap::Command),
            (Key::Char('B'), ActionMap::Bulk),
            (Key::Char('C'), ActionMap::Compress),
            (Key::Char('D'), ActionMap::Diff),
            (Key::Char('E'), ActionMap::EncryptedDrive),
            (Key::Char('F'), ActionMap::Filter),
            (Key::Char('G'), ActionMap::End),
            (Key::Char('H'), ActionMap::Help),
            (Key::Char('J'), ActionMap::PageUp),
            (Key::Char('K'), ActionMap::PageDown),
            (Key::Char('I'), ActionMap::NvimSetAddress),
            (Key::Char('L'), ActionMap::Symlink),
            (Key::Char('M'), ActionMap::MarksNew),
            (Key::Char('O'), ActionMap::Sort),
            (Key::Char('P'), ActionMap::Preview),
            (Key::Char('S'), ActionMap::ShellMenu),
            (Key::Char('T'), ActionMap::MediaInfo),
            (Key::Char('X'), ActionMap::TrashMoveFile),
            (Key::Char('W'), ActionMap::SetWallpaper),
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
            (Key::Alt('c'), ActionMap::OpenConfig),
            (Key::Alt('d'), ActionMap::DragNDrop),
            (Key::Alt('e'), ActionMap::ToggleDisplayFull),
            (Key::Alt('f'), ActionMap::ToggleDualPane),
            (Key::Alt('g'), ActionMap::Goto),
            (Key::Alt('h'), ActionMap::FuzzyFindHelp),
            (Key::Alt('i'), ActionMap::CliInfo),
            (Key::Alt('j'), ActionMap::Jump),
            (Key::Alt('l'), ActionMap::Log),
            (Key::Alt('o'), ActionMap::TrashOpen),
            (Key::Alt('p'), ActionMap::TogglePreviewSecond),
            (Key::Alt('r'), ActionMap::RemoteMount),
            (Key::Alt('x'), ActionMap::TrashEmpty),
            (Key::Alt('z'), ActionMap::TreeFoldAll),
            (Key::Ctrl('c'), ActionMap::CopyFilename),
            (Key::Ctrl('d'), ActionMap::PageDown),
            (Key::Ctrl('f'), ActionMap::FuzzyFind),
            (Key::Ctrl('g'), ActionMap::Shortcut),
            (Key::Ctrl('h'), ActionMap::History),
            (Key::Ctrl('s'), ActionMap::FuzzyFindLine),
            (Key::Ctrl('u'), ActionMap::PageUp),
            (Key::Ctrl('p'), ActionMap::CopyFilepath),
            (Key::Ctrl('q'), ActionMap::ResetMode),
            (Key::Ctrl('r'), ActionMap::RefreshView),
            (Key::Ctrl('x'), ActionMap::MocpClearPlaylist),
            (Key::AltEnter, ActionMap::MocpGoToSong),
            (Key::CtrlUp, ActionMap::MocpAddToPlayList),
            (Key::CtrlDown, ActionMap::MocpTogglePause),
            (Key::CtrlRight, ActionMap::MocpNext),
            (Key::CtrlLeft, ActionMap::MocpPrevious),
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
            .map(|(k, v)| (v.to_string(), format!("{k:?}")))
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
                log::info!("{CONFIG_PATH}: Keybinding {yaml_key:?} is unreadable");
                continue;
            };
            let Some(keymap) = from_keyname(key_string) else {
                log::info!("{CONFIG_PATH}: Keybinding {key_string} is unknown");
                continue;
            };
            if self.keymap_is_reserved(&keymap) {
                continue;
            }
            let Some(action_str) = yaml[yaml_key].as_str() else {
                continue;
            };
            let Ok(action) = ActionMap::from_str(action_str) else {
                log::info!("{CONFIG_PATH}: Action {action_str} is unknown");
                continue;
            };
            self.binds.insert(keymap, action);
        }
    }

    /// List of keymap used internally which can't be bound to anything.
    fn keymap_is_reserved(&self, keymap: &Key) -> bool {
        match *keymap {
            // used to send refresh requests.
            Key::AltPageUp => true,
            _ => false,
        }
    }

    pub fn update_custom(&mut self, yaml: &serde_yaml::value::Value) {
        let Some(mappings) = yaml.as_mapping() else {
            return;
        };
        let mut custom = vec![];
        for yaml_key in mappings.keys() {
            let Some(key_string) = yaml_key.as_str() else {
                log::info!("~/.config/fm/config.yaml: Keybinding {yaml_key:?} is unreadable");
                continue;
            };
            let Some(keymap) = from_keyname(key_string) else {
                log::info!("~/.config/fm/config.yaml: Keybinding {key_string} is unknown");
                continue;
            };
            let Some(custom_str) = yaml[yaml_key].as_str() else {
                continue;
            };
            let action = ActionMap::Custom(custom_str.to_owned());
            log::info!("custom bind {keymap:?}, {action}");
            self.binds.insert(keymap, action.clone());
            custom.push(format!("{keymap:?}:        {custom_str}\n"));
        }
        self.custom = Some(custom);
    }
}
