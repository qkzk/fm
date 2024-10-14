use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use crossterm::event::KeyCode;
use serde_yml::Value;

use crate::common::CONFIG_PATH;
use crate::event::ActionMap;
use crate::log_info;

// inspired by tuikit 0.5 : https://docs.rs/tuikit/latest/src/tuikit/key.rs.html#72-271
#[rustfmt::skip]
pub fn from_keyname(keyname: &str) -> Option<KeyCode> {
    match keyname.to_lowercase().as_ref() {
        "ctrl-space" | "ctrl-`" | "ctrl-@" => Some(KeyCode::Ctrl(' ')),
        "ctrl-a" => Some(KeyCode::Ctrl('a')),
        "ctrl-b" => Some(KeyCode::Ctrl('b')),
        "ctrl-c" => Some(KeyCode::Ctrl('c')),
        "ctrl-d" => Some(KeyCode::Ctrl('d')),
        "ctrl-e" => Some(KeyCode::Ctrl('e')),
        "ctrl-f" => Some(KeyCode::Ctrl('f')),
        "ctrl-g" => Some(KeyCode::Ctrl('g')),
        "ctrl-h" => Some(KeyCode::Ctrl('h')),
        "tab" | "ctrl-i" => Some(KeyCode::Tab),
        "ctrl-j" => Some(KeyCode::Ctrl('j')),
        "ctrl-k" => Some(KeyCode::Ctrl('k')),
        "ctrl-l" => Some(KeyCode::Ctrl('l')),
        "enter" | "return" | "ctrl-m" => Some(KeyCode::Enter),
        "ctrl-n" => Some(KeyCode::Ctrl('n')),
        "ctrl-o" => Some(KeyCode::Ctrl('o')),
        "ctrl-p" => Some(KeyCode::Ctrl('p')),
        "ctrl-q" => Some(KeyCode::Ctrl('q')),
        "ctrl-r" => Some(KeyCode::Ctrl('r')),
        "ctrl-s" => Some(KeyCode::Ctrl('s')),
        "ctrl-t" => Some(KeyCode::Ctrl('t')),
        "ctrl-u" => Some(KeyCode::Ctrl('u')),
        "ctrl-v" => Some(KeyCode::Ctrl('v')),
        "ctrl-w" => Some(KeyCode::Ctrl('w')),
        "ctrl-x" => Some(KeyCode::Ctrl('x')),
        "ctrl-y" => Some(KeyCode::Ctrl('y')),
        "ctrl-z" => Some(KeyCode::Ctrl('z')),
        "ctrl-up"    => Some(KeyCode::CtrlUp),
        "ctrl-down"  => Some(KeyCode::CtrlDown),
        "ctrl-left"  => Some(KeyCode::CtrlLeft),
        "ctrl-right" => Some(KeyCode::CtrlRight),

        "ctrl-alt-space" => Some(KeyCode::Ctrl(' ')),
        "ctrl-alt-a" => Some(KeyCode::CtrlAlt('a')),
        "ctrl-alt-b" => Some(KeyCode::CtrlAlt('b')),
        "ctrl-alt-c" => Some(KeyCode::CtrlAlt('c')),
        "ctrl-alt-d" => Some(KeyCode::CtrlAlt('d')),
        "ctrl-alt-e" => Some(KeyCode::CtrlAlt('e')),
        "ctrl-alt-f" => Some(KeyCode::CtrlAlt('f')),
        "ctrl-alt-g" => Some(KeyCode::CtrlAlt('g')),
        "ctrl-alt-h" => Some(KeyCode::CtrlAlt('h')),
        "ctrl-alt-j" => Some(KeyCode::CtrlAlt('j')),
        "ctrl-alt-k" => Some(KeyCode::CtrlAlt('k')),
        "ctrl-alt-l" => Some(KeyCode::CtrlAlt('l')),
        "ctrl-alt-n" => Some(KeyCode::CtrlAlt('n')),
        "ctrl-alt-o" => Some(KeyCode::CtrlAlt('o')),
        "ctrl-alt-p" => Some(KeyCode::CtrlAlt('p')),
        "ctrl-alt-q" => Some(KeyCode::CtrlAlt('q')),
        "ctrl-alt-r" => Some(KeyCode::CtrlAlt('r')),
        "ctrl-alt-s" => Some(KeyCode::CtrlAlt('s')),
        "ctrl-alt-t" => Some(KeyCode::CtrlAlt('t')),
        "ctrl-alt-u" => Some(KeyCode::CtrlAlt('u')),
        "ctrl-alt-v" => Some(KeyCode::CtrlAlt('v')),
        "ctrl-alt-w" => Some(KeyCode::CtrlAlt('w')),
        "ctrl-alt-x" => Some(KeyCode::CtrlAlt('x')),
        "ctrl-alt-y" => Some(KeyCode::CtrlAlt('y')),
        "ctrl-alt-z" => Some(KeyCode::CtrlAlt('z')),

        "esc"                => Some(KeyCode::ESC),
        "btab" | "shift-tab" => Some(KeyCode::BackTab),
        "bspace" | "bs"      => Some(KeyCode::Backspace),
        "ins" | "insert"     => Some(KeyCode::Insert),
        "del"                => Some(KeyCode::Delete),
        "pgup" | "page-up"   => Some(KeyCode::PageUp),
        "pgdn" | "page-down" => Some(KeyCode::PageDown),
        "up"                 => Some(KeyCode::Up),
        "down"               => Some(KeyCode::Down),
        "left"               => Some(KeyCode::Left),
        "right"              => Some(KeyCode::Right),
        "home"               => Some(KeyCode::Home),
        "end"                => Some(KeyCode::End),
        "shift-up"           => Some(KeyCode::ShiftUp),
        "shift-down"         => Some(KeyCode::ShiftDown),
        "shift-left"         => Some(KeyCode::ShiftLeft),
        "shift-right"        => Some(KeyCode::ShiftRight),

        "f1"  => Some(KeyCode::F(1)),
        "f2"  => Some(KeyCode::F(2)),
        "f3"  => Some(KeyCode::F(3)),
        "f4"  => Some(KeyCode::F(4)),
        "f5"  => Some(KeyCode::F(5)),
        "f6"  => Some(KeyCode::F(6)),
        "f7"  => Some(KeyCode::F(7)),
        "f8"  => Some(KeyCode::F(8)),
        "f9"  => Some(KeyCode::F(9)),
        "f10" => Some(KeyCode::F(10)),
        "f11" => Some(KeyCode::F(11)),
        "f12" => Some(KeyCode::F(12)),

        "alt-a" => Some(KeyCode::Alt('a')),
        "alt-b" => Some(KeyCode::Alt('b')),
        "alt-c" => Some(KeyCode::Alt('c')),
        "alt-d" => Some(KeyCode::Alt('d')),
        "alt-e" => Some(KeyCode::Alt('e')),
        "alt-f" => Some(KeyCode::Alt('f')),
        "alt-g" => Some(KeyCode::Alt('g')),
        "alt-h" => Some(KeyCode::Alt('h')),
        "alt-i" => Some(KeyCode::Alt('i')),
        "alt-j" => Some(KeyCode::Alt('j')),
        "alt-k" => Some(KeyCode::Alt('k')),
        "alt-l" => Some(KeyCode::Alt('l')),
        "alt-m" => Some(KeyCode::Alt('m')),
        "alt-n" => Some(KeyCode::Alt('n')),
        "alt-o" => Some(KeyCode::Alt('o')),
        "alt-p" => Some(KeyCode::Alt('p')),
        "alt-q" => Some(KeyCode::Alt('q')),
        "alt-r" => Some(KeyCode::Alt('r')),
        "alt-s" => Some(KeyCode::Alt('s')),
        "alt-t" => Some(KeyCode::Alt('t')),
        "alt-u" => Some(KeyCode::Alt('u')),
        "alt-v" => Some(KeyCode::Alt('v')),
        "alt-w" => Some(KeyCode::Alt('w')),
        "alt-x" => Some(KeyCode::Alt('x')),
        "alt-y" => Some(KeyCode::Alt('y')),
        "alt-z" => Some(KeyCode::Alt('z')),
        "alt-/" => Some(KeyCode::Alt('/')),

        "shift-a" => Some(KeyCode::Char('A')),
        "shift-b" => Some(KeyCode::Char('B')),
        "shift-c" => Some(KeyCode::Char('C')),
        "shift-d" => Some(KeyCode::Char('D')),
        "shift-e" => Some(KeyCode::Char('E')),
        "shift-f" => Some(KeyCode::Char('F')),
        "shift-g" => Some(KeyCode::Char('G')),
        "shift-h" => Some(KeyCode::Char('H')),
        "shift-i" => Some(KeyCode::Char('I')),
        "shift-j" => Some(KeyCode::Char('J')),
        "shift-k" => Some(KeyCode::Char('K')),
        "shift-l" => Some(KeyCode::Char('L')),
        "shift-m" => Some(KeyCode::Char('M')),
        "shift-n" => Some(KeyCode::Char('N')),
        "shift-o" => Some(KeyCode::Char('O')),
        "shift-p" => Some(KeyCode::Char('P')),
        "shift-q" => Some(KeyCode::Char('Q')),
        "shift-r" => Some(KeyCode::Char('R')),
        "shift-s" => Some(KeyCode::Char('S')),
        "shift-t" => Some(KeyCode::Char('T')),
        "shift-u" => Some(KeyCode::Char('U')),
        "shift-v" => Some(KeyCode::Char('V')),
        "shift-w" => Some(KeyCode::Char('W')),
        "shift-x" => Some(KeyCode::Char('X')),
        "shift-y" => Some(KeyCode::Char('Y')),
        "shift-z" => Some(KeyCode::Char('Z')),

        "alt-shift-a" => Some(KeyCode::Alt('A')),
        "alt-shift-b" => Some(KeyCode::Alt('B')),
        "alt-shift-c" => Some(KeyCode::Alt('C')),
        "alt-shift-d" => Some(KeyCode::Alt('D')),
        "alt-shift-e" => Some(KeyCode::Alt('E')),
        "alt-shift-f" => Some(KeyCode::Alt('F')),
        "alt-shift-g" => Some(KeyCode::Alt('G')),
        "alt-shift-h" => Some(KeyCode::Alt('H')),
        "alt-shift-i" => Some(KeyCode::Alt('I')),
        "alt-shift-j" => Some(KeyCode::Alt('J')),
        "alt-shift-k" => Some(KeyCode::Alt('K')),
        "alt-shift-l" => Some(KeyCode::Alt('L')),
        "alt-shift-m" => Some(KeyCode::Alt('M')),
        "alt-shift-n" => Some(KeyCode::Alt('N')),
        "alt-shift-o" => Some(KeyCode::Alt('O')),
        "alt-shift-p" => Some(KeyCode::Alt('P')),
        "alt-shift-q" => Some(KeyCode::Alt('Q')),
        "alt-shift-r" => Some(KeyCode::Alt('R')),
        "alt-shift-s" => Some(KeyCode::Alt('S')),
        "alt-shift-t" => Some(KeyCode::Alt('T')),
        "alt-shift-u" => Some(KeyCode::Alt('U')),
        "alt-shift-v" => Some(KeyCode::Alt('V')),
        "alt-shift-w" => Some(KeyCode::Alt('W')),
        "alt-shift-x" => Some(KeyCode::Alt('X')),
        "alt-shift-y" => Some(KeyCode::Alt('Y')),
        "alt-shift-z" => Some(KeyCode::Alt('Z')),

        "alt-btab" | "alt-shift-tab" => Some(KeyCode::AltBackTab),
        "alt-bspace" | "alt-bs"      => Some(KeyCode::AltBackspace),
        "alt-pgup" | "alt-page-up"   => Some(KeyCode::AltPageUp),
        "alt-pgdn" | "alt-page-down" => Some(KeyCode::AltPageDown),
        "alt-up"                     => Some(KeyCode::AltUp),
        "alt-down"                   => Some(KeyCode::AltDown),
        "alt-left"                   => Some(KeyCode::AltLeft),
        "alt-right"                  => Some(KeyCode::AltRight),
        "alt-home"                   => Some(KeyCode::AltHome),
        "alt-end"                    => Some(KeyCode::AltEnd),
        "alt-shift-up"               => Some(KeyCode::AltShiftUp),
        "alt-shift-down"             => Some(KeyCode::AltShiftDown),
        "alt-shift-left"             => Some(KeyCode::AltShiftLeft),
        "alt-shift-right"            => Some(KeyCode::AltShiftRight),
        "alt-enter" | "alt-ctrl-m"   => Some(KeyCode::AltEnter),
        "alt-tab" | "alt-ctrl-i"     => Some(KeyCode::AltTab),

        "space" => Some(KeyCode::Char(' ')),
        "alt-space" => Some(KeyCode::Alt(' ')),

        ch if ch.chars().count() == 1 => {
            Some(KeyCode::Char(ch.chars().next().expect("input:parse_key: no key is specified")))
        },
        _ => None,
    }
}

/// Holds an hashmap between keys and actions.
#[derive(Clone, Debug)]
pub struct Bindings {
    /// An HashMap of key & Actions.
    /// Every binded key is linked to its corresponding action
    pub binds: HashMap<KeyCode, ActionMap>,
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
            (KeyCode::ESC, ActionMap::ResetMode),
            (KeyCode::Up, ActionMap::MoveUp),
            (KeyCode::Down, ActionMap::MoveDown),
            (KeyCode::Left, ActionMap::MoveLeft),
            (KeyCode::Right, ActionMap::MoveRight),
            (KeyCode::Backspace, ActionMap::Backspace),
            (KeyCode::Delete, ActionMap::Delete),
            (KeyCode::Home, ActionMap::KeyHome),
            (KeyCode::End, ActionMap::End),
            (KeyCode::PageDown, ActionMap::PageDown),
            (KeyCode::PageUp, ActionMap::PageUp),
            (KeyCode::Enter, ActionMap::Enter),
            (KeyCode::Tab, ActionMap::Tab),
            (KeyCode::BackTab, ActionMap::Tab),
            (KeyCode::Char(' '), ActionMap::ToggleFlag),
            (KeyCode::Char('/'), ActionMap::Search),
            (KeyCode::Char('*'), ActionMap::FlagAll),
            (KeyCode::Char('\''), ActionMap::MarksJump),
            (KeyCode::Char('-'), ActionMap::Back),
            (KeyCode::Char('~'), ActionMap::Home),
            (KeyCode::Char('`'), ActionMap::GoRoot),
            (KeyCode::Char('!'), ActionMap::ShellCommand),
            (KeyCode::Char('@'), ActionMap::GoStart),
            (KeyCode::Char(':'), ActionMap::Action),
            (KeyCode::Char('6'), ActionMap::History),
            (KeyCode::Char('C'), ActionMap::Compress),
            (KeyCode::Char('E'), ActionMap::ToggleDisplayFull),
            (KeyCode::Char('G'), ActionMap::End),
            (KeyCode::Char('F'), ActionMap::DisplayFlagged),
            (KeyCode::Char('H'), ActionMap::FuzzyFindHelp),
            (KeyCode::Char('J'), ActionMap::PageDown),
            (KeyCode::Char('K'), ActionMap::PageUp),
            (KeyCode::Char('I'), ActionMap::NvimSetAddress),
            (KeyCode::Char('L'), ActionMap::Symlink),
            (KeyCode::Char('M'), ActionMap::MarksNew),
            (KeyCode::Char('O'), ActionMap::Sort),
            (KeyCode::Char('P'), ActionMap::Preview),
            (KeyCode::Char('X'), ActionMap::TrashMoveFile),
            (KeyCode::Char('a'), ActionMap::ToggleHidden),
            (KeyCode::Char('c'), ActionMap::CopyPaste),
            (KeyCode::Char('d'), ActionMap::NewDir),
            (KeyCode::Char('e'), ActionMap::Exec),
            (KeyCode::Char('f'), ActionMap::SearchNext),
            (KeyCode::Char('g'), ActionMap::KeyHome),
            (KeyCode::Char('k'), ActionMap::MoveUp),
            (KeyCode::Char('j'), ActionMap::MoveDown),
            (KeyCode::Char('h'), ActionMap::MoveLeft),
            (KeyCode::Char('l'), ActionMap::MoveRight),
            (KeyCode::Char('i'), ActionMap::NvimFilepicker),
            (KeyCode::Char('n'), ActionMap::NewFile),
            (KeyCode::Char('o'), ActionMap::OpenFile),
            (KeyCode::Char('m'), ActionMap::CutPaste),
            (KeyCode::Char('q'), ActionMap::Quit),
            (KeyCode::Char('r'), ActionMap::Rename),
            (KeyCode::Char('s'), ActionMap::Shell),
            (KeyCode::Char('t'), ActionMap::Tree),
            (KeyCode::Char('u'), ActionMap::ClearFlags),
            (KeyCode::Char('v'), ActionMap::ReverseFlags),
            (KeyCode::Char('w'), ActionMap::RegexMatch),
            (KeyCode::Char('x'), ActionMap::Delete),
            (KeyCode::Char('z'), ActionMap::TreeFold),
            (KeyCode::Char('Z'), ActionMap::TreeUnFoldAll),
            (KeyCode::Alt('b'), ActionMap::Bulk),
            (KeyCode::Alt('c'), ActionMap::OpenConfig),
            (KeyCode::Alt('C'), ActionMap::CloudDrive),
            (KeyCode::Alt('d'), ActionMap::ToggleDualPane),
            (KeyCode::Alt('e'), ActionMap::EncryptedDrive),
            (KeyCode::Alt('f'), ActionMap::Filter),
            (KeyCode::Alt('g'), ActionMap::Cd),
            (KeyCode::Alt('h'), ActionMap::Help),
            (KeyCode::Alt('i'), ActionMap::CliMenu),
            (KeyCode::Alt('l'), ActionMap::Log),
            (KeyCode::Alt('o'), ActionMap::TrashOpen),
            (KeyCode::Alt('r'), ActionMap::RemoteMount),
            (KeyCode::Alt('s'), ActionMap::TuiMenu),
            (KeyCode::Alt('R'), ActionMap::RemovableDevices),
            (KeyCode::Alt('t'), ActionMap::Context),
            (KeyCode::Alt('x'), ActionMap::TrashEmpty),
            (KeyCode::Alt('m'), ActionMap::Chmod),
            (KeyCode::Alt('p'), ActionMap::TogglePreviewSecond),
            (KeyCode::Ctrl('c'), ActionMap::CopyFilename),
            (KeyCode::Ctrl('d'), ActionMap::PageDown),
            (KeyCode::Ctrl('f'), ActionMap::FuzzyFind),
            (KeyCode::Ctrl('g'), ActionMap::Shortcut),
            (KeyCode::Ctrl('s'), ActionMap::FuzzyFindLine),
            (KeyCode::Ctrl('u'), ActionMap::PageUp),
            (KeyCode::Ctrl('o'), ActionMap::OpenAll),
            (KeyCode::Ctrl('p'), ActionMap::CopyFilepath),
            (KeyCode::Ctrl('q'), ActionMap::ResetMode),
            (KeyCode::Ctrl('r'), ActionMap::RefreshView),
            (KeyCode::Ctrl('z'), ActionMap::TreeFoldAll),
            (KeyCode::ShiftRight, ActionMap::SyncLTR),
            (KeyCode::ShiftDown, ActionMap::NextThing),
            (KeyCode::ShiftLeft, ActionMap::DeleteLine),
            (KeyCode::ShiftUp, ActionMap::PreviousThing),
            (KeyCode::CtrlUp, ActionMap::FocusGoUp),
            (KeyCode::CtrlDown, ActionMap::FocusGoDown),
            (KeyCode::CtrlRight, ActionMap::FocusGoRight),
            (KeyCode::CtrlLeft, ActionMap::FocusGoLeft),
            (KeyCode::Ctrl('h'), ActionMap::FocusGoLeft),
            (KeyCode::Ctrl('j'), ActionMap::FocusGoDown),
            (KeyCode::Ctrl('k'), ActionMap::FocusGoUp),
            (KeyCode::Ctrl('l'), ActionMap::FocusGoRight),
            (KeyCode::F(1), ActionMap::FuzzyFindHelp),
            (KeyCode::F(2), ActionMap::Rename),
            (KeyCode::F(3), ActionMap::Preview),
            (KeyCode::F(4), ActionMap::OpenFile),
            (KeyCode::F(5), ActionMap::CopyPaste),
            (KeyCode::F(6), ActionMap::CutPaste),
            (KeyCode::F(7), ActionMap::NewDir),
            (KeyCode::F(8), ActionMap::Delete),
            (KeyCode::F(9), ActionMap::NewFile),
            (KeyCode::F(10), ActionMap::Quit),
            (KeyCode::F(11), ActionMap::FlaggedToClipboard),
            (KeyCode::F(12), ActionMap::FlaggedFromClipboard),
        ]);
        let custom = None;
        Self { binds, custom }
    }

    /// Returns an Option of action. None if the key isn't binded.
    pub fn get(&self, key: &KeyCode) -> Option<&ActionMap> {
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
    pub fn update_normal(&mut self, yaml: &Value) {
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

    pub fn update_custom(&mut self, yaml: &Value) {
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

    /// Format all keybindings in alphabetical order.
    pub fn to_str(&self) -> String {
        let mut binds = vec![];
        for (key, action) in self.binds.iter() {
            binds.push(format!(
                "{key:?}:         {action} - {desc}\n",
                desc = action.description()
            ))
        }
        binds.sort();
        let binds = binds.join("");

        let keybinds_string = format!("fm keybindings \n\n{binds}");
        keybinds_string
    }
}
