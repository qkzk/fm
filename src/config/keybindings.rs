use std::collections::HashMap;
use std::str::FromStr;
use std::string::ToString;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_yml::Value;

use crate::common::CONFIG_PATH;
use crate::event::ActionMap;
use crate::log_info;

/// Used to parse keynames from config file into [`crossterm::event::KeyEvent`].
/// Inspired by tuikit 0.5 : <https://github.com/lotabout/skim-rs/blob/master/src/key.rs#L72-L271>
#[rustfmt::skip]
pub fn from_keyname(keyname: &str) -> Option<KeyEvent> {
    match keyname.to_lowercase().as_ref() {
        "enter" | "return" | "ctrl-m"       => Some(KeyEvent::new(KeyCode::Enter,     KeyModifiers::NONE)),
        "tab" | "ctrl-i"                    => Some(KeyEvent::new(KeyCode::Tab,       KeyModifiers::NONE)),
        "esc"                               => Some(KeyEvent::new(KeyCode::Esc,       KeyModifiers::NONE)),
        "btab" | "shift-tab"                => Some(KeyEvent::new(KeyCode::BackTab,   KeyModifiers::NONE)),
        "bspace" | "bs"                     => Some(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        "ins" | "insert"                    => Some(KeyEvent::new(KeyCode::Insert,    KeyModifiers::NONE)),
        "del"                               => Some(KeyEvent::new(KeyCode::Delete,    KeyModifiers::NONE)),
        "pgup" | "page-up"                  => Some(KeyEvent::new(KeyCode::PageUp,    KeyModifiers::NONE)),
        "pgdn" | "page-down"                => Some(KeyEvent::new(KeyCode::PageDown,  KeyModifiers::NONE)),
        "up"                                => Some(KeyEvent::new(KeyCode::Up,        KeyModifiers::NONE)),
        "down"                              => Some(KeyEvent::new(KeyCode::Down,      KeyModifiers::NONE)),
        "left"                              => Some(KeyEvent::new(KeyCode::Left,      KeyModifiers::NONE)),
        "right"                             => Some(KeyEvent::new(KeyCode::Right,     KeyModifiers::NONE)),
        "home"                              => Some(KeyEvent::new(KeyCode::Home,      KeyModifiers::NONE)),
        "end"                               => Some(KeyEvent::new(KeyCode::End,       KeyModifiers::NONE)),

        "shift-up"                          => Some(KeyEvent::new(KeyCode::Up,        KeyModifiers::SHIFT)),
        "shift-down"                        => Some(KeyEvent::new(KeyCode::Down,      KeyModifiers::SHIFT)),
        "shift-left"                        => Some(KeyEvent::new(KeyCode::Left,      KeyModifiers::SHIFT)),
        "shift-right"                       => Some(KeyEvent::new(KeyCode::Right,     KeyModifiers::SHIFT)),

        "ctrl-space" | "ctrl-`" | "ctrl-@"  => Some(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL)),
        "ctrl-a"                            => Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
        "ctrl-b"                            => Some(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL)),
        "ctrl-c"                            => Some(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        "ctrl-d"                            => Some(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        "ctrl-e"                            => Some(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)),
        "ctrl-f"                            => Some(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)),
        "ctrl-g"                            => Some(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)),
        "ctrl-h"                            => Some(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL)),
        "ctrl-j"                            => Some(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL)),
        "ctrl-k"                            => Some(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)),
        "ctrl-l"                            => Some(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
        "ctrl-n"                            => Some(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL)),
        "ctrl-o"                            => Some(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
        "ctrl-p"                            => Some(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
        "ctrl-q"                            => Some(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
        "ctrl-r"                            => Some(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
        "ctrl-s"                            => Some(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
        "ctrl-t"                            => Some(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
        "ctrl-u"                            => Some(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        "ctrl-v"                            => Some(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)),
        "ctrl-w"                            => Some(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
        "ctrl-x"                            => Some(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
        "ctrl-y"                            => Some(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL)),
        "ctrl-z"                            => Some(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)),

        "ctrl-up"                           => Some(KeyEvent::new(KeyCode::Up,        KeyModifiers::CONTROL)),
        "ctrl-down"                         => Some(KeyEvent::new(KeyCode::Down,      KeyModifiers::CONTROL)),
        "ctrl-left"                         => Some(KeyEvent::new(KeyCode::Left,      KeyModifiers::CONTROL)),
        "ctrl-right"                        => Some(KeyEvent::new(KeyCode::Right,     KeyModifiers::CONTROL)),

        "ctrl-tab"                          => Some(KeyEvent::new(KeyCode::Tab,       KeyModifiers::CONTROL)),

        "ctrl-alt-space"                    => Some(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-a"                        => Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-b"                        => Some(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-c"                        => Some(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-d"                        => Some(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-e"                        => Some(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-f"                        => Some(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-g"                        => Some(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-h"                        => Some(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-j"                        => Some(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-k"                        => Some(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-l"                        => Some(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-n"                        => Some(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-o"                        => Some(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-p"                        => Some(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-q"                        => Some(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-r"                        => Some(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-s"                        => Some(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-t"                        => Some(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-u"                        => Some(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-v"                        => Some(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-w"                        => Some(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-x"                        => Some(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-y"                        => Some(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
        "ctrl-alt-z"                        => Some(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL | KeyModifiers::ALT)),

        "f1"                                => Some(KeyEvent::new(KeyCode::F(1),      KeyModifiers::NONE)),
        "f2"                                => Some(KeyEvent::new(KeyCode::F(2),      KeyModifiers::NONE)),
        "f3"                                => Some(KeyEvent::new(KeyCode::F(3),      KeyModifiers::NONE)),
        "f4"                                => Some(KeyEvent::new(KeyCode::F(4),      KeyModifiers::NONE)),
        "f5"                                => Some(KeyEvent::new(KeyCode::F(5),      KeyModifiers::NONE)),
        "f6"                                => Some(KeyEvent::new(KeyCode::F(6),      KeyModifiers::NONE)),
        "f7"                                => Some(KeyEvent::new(KeyCode::F(7),      KeyModifiers::NONE)),
        "f8"                                => Some(KeyEvent::new(KeyCode::F(8),      KeyModifiers::NONE)),
        "f9"                                => Some(KeyEvent::new(KeyCode::F(9),      KeyModifiers::NONE)),
        "f10"                               => Some(KeyEvent::new(KeyCode::F(10),     KeyModifiers::NONE)),
        "f11"                               => Some(KeyEvent::new(KeyCode::F(11),     KeyModifiers::NONE)),
        "f12"                               => Some(KeyEvent::new(KeyCode::F(12),     KeyModifiers::NONE)),


        "alt-a"                             => Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT)),
        "alt-b"                             => Some(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT)),
        "alt-c"                             => Some(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::ALT)),
        "alt-d"                             => Some(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::ALT)),
        "alt-e"                             => Some(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT)),
        "alt-f"                             => Some(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT)),
        "alt-g"                             => Some(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT)),
        "alt-h"                             => Some(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT)),
        "alt-i"                             => Some(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::ALT)),
        "alt-j"                             => Some(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::ALT)),
        "alt-k"                             => Some(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::ALT)),
        "alt-l"                             => Some(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::ALT)),
        "alt-m"                             => Some(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT)),
        "alt-n"                             => Some(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::ALT)),
        "alt-o"                             => Some(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::ALT)),
        "alt-p"                             => Some(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::ALT)),
        "alt-q"                             => Some(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT)),
        "alt-r"                             => Some(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT)),
        "alt-s"                             => Some(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::ALT)),
        "alt-t"                             => Some(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT)),
        "alt-u"                             => Some(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::ALT)),
        "alt-v"                             => Some(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::ALT)),
        "alt-w"                             => Some(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::ALT)),
        "alt-x"                             => Some(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT)),
        "alt-y"                             => Some(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::ALT)),
        "alt-z"                             => Some(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::ALT)),
        "alt-/"                             => Some(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::ALT)),

        "shift-a"                           => Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT)),
        "shift-b"                           => Some(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::SHIFT)),
        "shift-c"                           => Some(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SHIFT)),
        "shift-d"                           => Some(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::SHIFT)),
        "shift-e"                           => Some(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::SHIFT)),
        "shift-f"                           => Some(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::SHIFT)),
        "shift-g"                           => Some(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::SHIFT)),
        "shift-h"                           => Some(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::SHIFT)),
        "shift-i"                           => Some(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::SHIFT)),
        "shift-j"                           => Some(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SHIFT)),
        "shift-k"                           => Some(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SHIFT)),
        "shift-l"                           => Some(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SHIFT)),
        "shift-m"                           => Some(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::SHIFT)),
        "shift-n"                           => Some(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::SHIFT)),
        "shift-o"                           => Some(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::SHIFT)),
        "shift-p"                           => Some(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::SHIFT)),
        "shift-q"                           => Some(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::SHIFT)),
        "shift-r"                           => Some(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::SHIFT)),
        "shift-s"                           => Some(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::SHIFT)),
        "shift-t"                           => Some(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::SHIFT)),
        "shift-u"                           => Some(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::SHIFT)),
        "shift-v"                           => Some(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SHIFT)),
        "shift-w"                           => Some(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::SHIFT)),
        "shift-x"                           => Some(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::SHIFT)),
        "shift-y"                           => Some(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::SHIFT)),
        "shift-z"                           => Some(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::SHIFT)),

        "alt-shift-a"                       => Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-b"                       => Some(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-c"                       => Some(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-d"                       => Some(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-e"                       => Some(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-f"                       => Some(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-g"                       => Some(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-h"                       => Some(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-i"                       => Some(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-j"                       => Some(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-k"                       => Some(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-l"                       => Some(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-m"                       => Some(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-n"                       => Some(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-o"                       => Some(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-p"                       => Some(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-q"                       => Some(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-r"                       => Some(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-s"                       => Some(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-t"                       => Some(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-u"                       => Some(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-v"                       => Some(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-w"                       => Some(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-x"                       => Some(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-y"                       => Some(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::SHIFT | KeyModifiers::ALT)),
        "alt-shift-z"                       => Some(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::SHIFT | KeyModifiers::ALT)),

        "alt-btab" | "alt-shift-tab"        => Some(KeyEvent::new(KeyCode::BackTab,   KeyModifiers::ALT)),
        "alt-bspace" | "alt-bs"             => Some(KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT)),
        "alt-pgup" | "alt-page-up"          => Some(KeyEvent::new(KeyCode::PageUp,    KeyModifiers::ALT)),
        "alt-pgdn" | "alt-page-down"        => Some(KeyEvent::new(KeyCode::PageDown,  KeyModifiers::ALT)),
        "alt-up"                            => Some(KeyEvent::new(KeyCode::Up,        KeyModifiers::ALT)),
        "alt-down"                          => Some(KeyEvent::new(KeyCode::Down,      KeyModifiers::ALT)),
        "alt-left"                          => Some(KeyEvent::new(KeyCode::Left,      KeyModifiers::ALT)),
        "alt-right"                         => Some(KeyEvent::new(KeyCode::Right,     KeyModifiers::ALT)),
        "alt-home"                          => Some(KeyEvent::new(KeyCode::Home,      KeyModifiers::ALT)),
        "alt-end"                           => Some(KeyEvent::new(KeyCode::End,       KeyModifiers::ALT)),
        "alt-shift-up"                      => Some(KeyEvent::new(KeyCode::Up,        KeyModifiers::ALT | KeyModifiers::SHIFT)),
        "alt-shift-down"                    => Some(KeyEvent::new(KeyCode::Down,      KeyModifiers::ALT | KeyModifiers::SHIFT)),
        "alt-shift-left"                    => Some(KeyEvent::new(KeyCode::Left,      KeyModifiers::ALT | KeyModifiers::SHIFT)),
        "alt-shift-right"                   => Some(KeyEvent::new(KeyCode::Right,     KeyModifiers::ALT | KeyModifiers::SHIFT)),
        "alt-enter" | "alt-ctrl-m"          => Some(KeyEvent::new(KeyCode::Enter,     KeyModifiers::ALT)),
        "alt-tab" | "alt-ctrl-i"            => Some(KeyEvent::new(KeyCode::Tab,       KeyModifiers::ALT)),

        ch if ch.chars().count() == 1 => {
            let char = ch.chars().next().expect("input:parse_key: no key is specified");
                                              Some(KeyEvent::new(KeyCode::Char(char), KeyModifiers::NONE))
        },
        _                                   => None
    }
}

/// Holds an hashmap between keys and actions.
#[derive(Clone, Debug)]
pub struct Bindings {
    /// An HashMap of key & Actions.
    /// Every binded key is linked to its corresponding action
    pub binds: HashMap<KeyEvent, ActionMap>,
    /// Remember every key binded to a custom action
    pub custom: Option<Vec<String>>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Bindings {
    #[rustfmt::skip]
    pub fn new() -> Self {
        let binds = HashMap::from([
            (KeyEvent::new(KeyCode::Esc,          KeyModifiers::NONE), ActionMap::ResetMode),
            (KeyEvent::new(KeyCode::Insert,       KeyModifiers::NONE), ActionMap::ResetMode),
            (KeyEvent::new(KeyCode::Up,           KeyModifiers::NONE), ActionMap::MoveUp),
            (KeyEvent::new(KeyCode::Down,         KeyModifiers::NONE), ActionMap::MoveDown),
            (KeyEvent::new(KeyCode::Left,         KeyModifiers::NONE), ActionMap::MoveLeft),
            (KeyEvent::new(KeyCode::Right,        KeyModifiers::NONE), ActionMap::MoveRight),
            (KeyEvent::new(KeyCode::Backspace,    KeyModifiers::NONE), ActionMap::Backspace),
            (KeyEvent::new(KeyCode::Delete,       KeyModifiers::NONE), ActionMap::Delete),
            (KeyEvent::new(KeyCode::Home,         KeyModifiers::NONE), ActionMap::KeyHome),
            (KeyEvent::new(KeyCode::End,          KeyModifiers::NONE), ActionMap::End),
            (KeyEvent::new(KeyCode::PageDown,     KeyModifiers::NONE), ActionMap::PageDown),
            (KeyEvent::new(KeyCode::PageUp,       KeyModifiers::NONE), ActionMap::PageUp),
            (KeyEvent::new(KeyCode::Enter,        KeyModifiers::NONE), ActionMap::Enter),
            (KeyEvent::new(KeyCode::Tab,          KeyModifiers::NONE), ActionMap::Tab),
            (KeyEvent::new(KeyCode::BackTab,      KeyModifiers::NONE), ActionMap::Tab),

            (KeyEvent::new(KeyCode::Char(' '),    KeyModifiers::NONE), ActionMap::ToggleFlag),
            (KeyEvent::new(KeyCode::Char('/'),    KeyModifiers::NONE), ActionMap::Search),
            (KeyEvent::new(KeyCode::Char('*'),    KeyModifiers::NONE), ActionMap::FlagAll),
            (KeyEvent::new(KeyCode::Char('\''),   KeyModifiers::NONE), ActionMap::MarksJump),
            (KeyEvent::new(KeyCode::Char('"'),    KeyModifiers::NONE), ActionMap::TempMarksJump),
            (KeyEvent::new(KeyCode::Char('-'),    KeyModifiers::NONE), ActionMap::Back),
            (KeyEvent::new(KeyCode::Char('~'),    KeyModifiers::NONE), ActionMap::Home),
            (KeyEvent::new(KeyCode::Char('`'),    KeyModifiers::NONE), ActionMap::GoRoot),
            (KeyEvent::new(KeyCode::Char('!'),    KeyModifiers::NONE), ActionMap::ShellCommand),
            (KeyEvent::new(KeyCode::Char('@'),    KeyModifiers::NONE), ActionMap::GoStart),
            (KeyEvent::new(KeyCode::Char(':'),    KeyModifiers::NONE), ActionMap::Action),
            (KeyEvent::new(KeyCode::Char('6'),    KeyModifiers::NONE), ActionMap::History),

            (KeyEvent::new(KeyCode::Char('c'),    KeyModifiers::SHIFT), ActionMap::OpenConfig),
            (KeyEvent::new(KeyCode::Char('e'),    KeyModifiers::SHIFT), ActionMap::ToggleDisplayFull),
            (KeyEvent::new(KeyCode::Char('g'),    KeyModifiers::SHIFT), ActionMap::End),
            (KeyEvent::new(KeyCode::Char('f'),    KeyModifiers::SHIFT), ActionMap::DisplayFlagged),
            (KeyEvent::new(KeyCode::Char('h'),    KeyModifiers::SHIFT), ActionMap::FuzzyFindHelp),
            (KeyEvent::new(KeyCode::Char('j'),    KeyModifiers::SHIFT), ActionMap::PageDown),
            (KeyEvent::new(KeyCode::Char('k'),    KeyModifiers::SHIFT), ActionMap::PageUp),
            (KeyEvent::new(KeyCode::Char('i'),    KeyModifiers::SHIFT), ActionMap::NvimSetAddress),
            (KeyEvent::new(KeyCode::Char('l'),    KeyModifiers::SHIFT), ActionMap::Symlink),
            (KeyEvent::new(KeyCode::Char('m'),    KeyModifiers::SHIFT), ActionMap::MarksNew),
            (KeyEvent::new(KeyCode::Char('o'),    KeyModifiers::SHIFT), ActionMap::Sort),
            (KeyEvent::new(KeyCode::Char('p'),    KeyModifiers::SHIFT), ActionMap::Preview),
            (KeyEvent::new(KeyCode::Char('v'),    KeyModifiers::SHIFT), ActionMap::ToggleVisual),
            (KeyEvent::new(KeyCode::Char('x'),    KeyModifiers::SHIFT), ActionMap::TrashMoveFile),
            (KeyEvent::new(KeyCode::Char('Z'),    KeyModifiers::SHIFT), ActionMap::TreeUnFoldAll),

            (KeyEvent::new(KeyCode::Char('a'),    KeyModifiers::NONE), ActionMap::ToggleHidden),
            (KeyEvent::new(KeyCode::Char('c'),    KeyModifiers::NONE), ActionMap::CopyPaste),
            (KeyEvent::new(KeyCode::Char('d'),    KeyModifiers::NONE), ActionMap::NewDir),
            (KeyEvent::new(KeyCode::Char('e'),    KeyModifiers::NONE), ActionMap::Exec),
            (KeyEvent::new(KeyCode::Char('f'),    KeyModifiers::NONE), ActionMap::SearchNext),
            (KeyEvent::new(KeyCode::Char('g'),    KeyModifiers::NONE), ActionMap::KeyHome),
            (KeyEvent::new(KeyCode::Char('k'),    KeyModifiers::NONE), ActionMap::MoveUp),
            (KeyEvent::new(KeyCode::Char('j'),    KeyModifiers::NONE), ActionMap::MoveDown),
            (KeyEvent::new(KeyCode::Char('h'),    KeyModifiers::NONE), ActionMap::MoveLeft),
            (KeyEvent::new(KeyCode::Char('l'),    KeyModifiers::NONE), ActionMap::MoveRight),
            (KeyEvent::new(KeyCode::Char('i'),    KeyModifiers::NONE), ActionMap::NvimFilepicker),
            (KeyEvent::new(KeyCode::Char('n'),    KeyModifiers::NONE), ActionMap::NewFile),
            (KeyEvent::new(KeyCode::Char('o'),    KeyModifiers::NONE), ActionMap::OpenFile),
            (KeyEvent::new(KeyCode::Char('m'),    KeyModifiers::NONE), ActionMap::CutPaste),
            (KeyEvent::new(KeyCode::Char('q'),    KeyModifiers::NONE), ActionMap::Quit),
            (KeyEvent::new(KeyCode::Char('r'),    KeyModifiers::NONE), ActionMap::Rename),
            (KeyEvent::new(KeyCode::Char('s'),    KeyModifiers::NONE), ActionMap::Shell),
            (KeyEvent::new(KeyCode::Char('t'),    KeyModifiers::NONE), ActionMap::Tree),
            (KeyEvent::new(KeyCode::Char('u'),    KeyModifiers::NONE), ActionMap::ClearFlags),
            (KeyEvent::new(KeyCode::Char('v'),    KeyModifiers::NONE), ActionMap::ReverseFlags),
            (KeyEvent::new(KeyCode::Char('w'),    KeyModifiers::NONE), ActionMap::RegexMatch),
            (KeyEvent::new(KeyCode::Char('x'),    KeyModifiers::NONE), ActionMap::Delete),
            (KeyEvent::new(KeyCode::Char('z'),    KeyModifiers::NONE), ActionMap::TreeFold),

            (KeyEvent::new(KeyCode::Char('+'),    KeyModifiers::NONE), ActionMap::Chmod),

            (KeyEvent::new(KeyCode::Char('b'),    KeyModifiers::ALT), ActionMap::Bulk),
            (KeyEvent::new(KeyCode::Char('c'),    KeyModifiers::ALT), ActionMap::Compress),
            (KeyEvent::new(KeyCode::Char('d'),    KeyModifiers::ALT), ActionMap::ToggleDualPane),
            (KeyEvent::new(KeyCode::Char('e'),    KeyModifiers::ALT), ActionMap::Mount),
            (KeyEvent::new(KeyCode::Char('f'),    KeyModifiers::ALT), ActionMap::Filter),
            (KeyEvent::new(KeyCode::Char('g'),    KeyModifiers::ALT), ActionMap::Cd),
            (KeyEvent::new(KeyCode::Char('h'),    KeyModifiers::ALT), ActionMap::Help),
            (KeyEvent::new(KeyCode::Char('i'),    KeyModifiers::ALT), ActionMap::CliMenu),
            (KeyEvent::new(KeyCode::Char('l'),    KeyModifiers::ALT), ActionMap::Log),
            (KeyEvent::new(KeyCode::Char('m'),    KeyModifiers::ALT), ActionMap::Chmod),
            (KeyEvent::new(KeyCode::Char('o'),    KeyModifiers::ALT), ActionMap::TrashOpen),
            (KeyEvent::new(KeyCode::Char('p'),    KeyModifiers::ALT), ActionMap::TogglePreviewSecond),
            (KeyEvent::new(KeyCode::Char('r'),    KeyModifiers::ALT), ActionMap::RemoteMount),
            (KeyEvent::new(KeyCode::Char('s'),    KeyModifiers::ALT), ActionMap::TuiMenu),
            (KeyEvent::new(KeyCode::Char('t'),    KeyModifiers::ALT), ActionMap::Context),
            (KeyEvent::new(KeyCode::Char('u'),    KeyModifiers::ALT), ActionMap::Mount),
            (KeyEvent::new(KeyCode::Char('x'),    KeyModifiers::ALT), ActionMap::TrashEmpty),
            (KeyEvent::new(KeyCode::Char('"'),    KeyModifiers::ALT), ActionMap::TempMarksNew),
            (KeyEvent::new(KeyCode::Char('\''),   KeyModifiers::ALT), ActionMap::MarksNew),

            (KeyEvent::new(KeyCode::Tab,          KeyModifiers::ALT),  ActionMap::ResetMode),
            (KeyEvent::new(KeyCode::Backspace,    KeyModifiers::ALT), ActionMap::DeleteLeft),

            (KeyEvent::new(KeyCode::Char('c'),    KeyModifiers::ALT | KeyModifiers::SHIFT), ActionMap::CloudDrive),

            (KeyEvent::new(KeyCode::Char('a'),    KeyModifiers::CONTROL), ActionMap::CopyContent),
            (KeyEvent::new(KeyCode::Char('c'),    KeyModifiers::CONTROL), ActionMap::CopyFilename),
            (KeyEvent::new(KeyCode::Char('d'),    KeyModifiers::CONTROL), ActionMap::PageDown),
            (KeyEvent::new(KeyCode::Char('f'),    KeyModifiers::CONTROL), ActionMap::FuzzyFind),
            (KeyEvent::new(KeyCode::Char('g'),    KeyModifiers::CONTROL), ActionMap::Shortcut),
            (KeyEvent::new(KeyCode::Char('s'),    KeyModifiers::CONTROL), ActionMap::FuzzyFindLine),
            (KeyEvent::new(KeyCode::Char('u'),    KeyModifiers::CONTROL), ActionMap::PageUp),
            (KeyEvent::new(KeyCode::Char('o'),    KeyModifiers::CONTROL), ActionMap::OpenAll),
            (KeyEvent::new(KeyCode::Char('p'),    KeyModifiers::CONTROL), ActionMap::CopyFilepath),
            (KeyEvent::new(KeyCode::Char('q'),    KeyModifiers::CONTROL), ActionMap::ResetMode),
            (KeyEvent::new(KeyCode::Char('r'),    KeyModifiers::CONTROL), ActionMap::RefreshView),
            (KeyEvent::new(KeyCode::Char('z'),    KeyModifiers::CONTROL), ActionMap::TreeFoldAll),

            (KeyEvent::new(KeyCode::Right,        KeyModifiers::SHIFT), ActionMap::SyncLTR),
            (KeyEvent::new(KeyCode::Down,         KeyModifiers::SHIFT), ActionMap::NextThing),
            (KeyEvent::new(KeyCode::Left,         KeyModifiers::SHIFT), ActionMap::DeleteLine),
            (KeyEvent::new(KeyCode::Up,           KeyModifiers::SHIFT), ActionMap::PreviousThing),

            (KeyEvent::new(KeyCode::Up,           KeyModifiers::CONTROL), ActionMap::FocusGoUp),
            (KeyEvent::new(KeyCode::Down,         KeyModifiers::CONTROL), ActionMap::FocusGoDown),
            (KeyEvent::new(KeyCode::Right,        KeyModifiers::CONTROL), ActionMap::FocusGoRight),
            (KeyEvent::new(KeyCode::Left,         KeyModifiers::CONTROL), ActionMap::FocusGoLeft),

            (KeyEvent::new(KeyCode::Char(' '),    KeyModifiers::CONTROL), ActionMap::ToggleFlagChildren),
            (KeyEvent::new(KeyCode::Char('h'),    KeyModifiers::CONTROL), ActionMap::FocusGoLeft),
            (KeyEvent::new(KeyCode::Char('j'),    KeyModifiers::CONTROL), ActionMap::FocusGoDown),
            (KeyEvent::new(KeyCode::Char('k'),    KeyModifiers::CONTROL), ActionMap::FocusGoUp),
            (KeyEvent::new(KeyCode::Char('l'),    KeyModifiers::CONTROL), ActionMap::FocusGoRight),
 
            (KeyEvent::new(KeyCode::F(1),         KeyModifiers::NONE), ActionMap::FuzzyFindHelp),
            (KeyEvent::new(KeyCode::F(2),         KeyModifiers::NONE), ActionMap::Rename),
            (KeyEvent::new(KeyCode::F(3),         KeyModifiers::NONE), ActionMap::Preview),
            (KeyEvent::new(KeyCode::F(4),         KeyModifiers::NONE), ActionMap::OpenFile),
            (KeyEvent::new(KeyCode::F(5),         KeyModifiers::NONE), ActionMap::CopyPaste),
            (KeyEvent::new(KeyCode::F(6),         KeyModifiers::NONE), ActionMap::CutPaste),
            (KeyEvent::new(KeyCode::F(7),         KeyModifiers::NONE), ActionMap::NewDir),
            (KeyEvent::new(KeyCode::F(8),         KeyModifiers::NONE), ActionMap::Delete),
            (KeyEvent::new(KeyCode::F(9),         KeyModifiers::NONE), ActionMap::NewFile),
            (KeyEvent::new(KeyCode::F(10),        KeyModifiers::NONE), ActionMap::Quit),
            (KeyEvent::new(KeyCode::F(11),        KeyModifiers::NONE), ActionMap::FlaggedToClipboard),
            (KeyEvent::new(KeyCode::F(12),        KeyModifiers::NONE), ActionMap::FlaggedFromClipboard),
        ]);
        let custom = None;
        Self { binds, custom }
    }

    /// Returns an Option of action. None if the key isn't binded.
    pub fn get(&self, key_event: &KeyEvent) -> Option<&ActionMap> {
        self.binds.get(key_event)
    }

    /// Reverse the hashmap of keys.
    /// Used to format the help string.
    pub fn keybind_reversed(&self) -> HashMap<String, String> {
        self.binds
            .iter()
            .map(|(keybind, action)| (action.to_string(), keybind.for_help().to_string()))
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
            let Some(key_event) = from_keyname(key_string) else {
                log_info!("~/.config/fm/config.yaml: Keybinding {key_string} is unknown");
                continue;
            };
            let Some(custom_str) = yaml[yaml_key].as_str() else {
                continue;
            };
            let action = ActionMap::Custom(custom_str.to_owned());
            log_info!("custom bind {key_event:?}, {custom_str}");
            self.binds.insert(key_event, action.clone());
            custom.push(format!("{kmh}:        {custom_str}\n", kmh=key_event.for_help()));
        }
        self.custom = Some(custom);
    }

    /// Format all keybindings in alphabetical order.
    pub fn to_str(&self) -> String {
        let mut binds = vec![];
        for (key, action) in self.binds.iter() {
            binds.push(format!(
                "{key}:         {action} - {desc}\n",
                key=key.for_help(),
                desc = action.description()
            ))
        }
        binds.sort();
        let binds = binds.join("");

        let keybinds_string = format!("fm keybindings \n\n{binds}");
        keybinds_string
    }
}

trait ForHelp {
    fn for_help(&self) -> String;
}

impl ForHelp for KeyEvent {
    fn for_help(&self) -> String {
        let KeyEvent{code, modifiers, kind: _, state: _} = self;
        let prefix = match *modifiers {
            KeyModifiers::SHIFT => "shift-",
            KeyModifiers::CONTROL => "ctrl-",
            KeyModifiers::ALT => "alt-",
            _ => "",
        };
        let scode = match *code {
            KeyCode::Char(' ') => "<SPC>".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(u) => format!("f{u}"),
            KeyCode::Enter => "enter".to_string(),
            KeyCode::Tab => "tab".to_string(),
            KeyCode::Esc => "esc".to_string(),
            KeyCode::BackTab => "tab".to_string(),
            KeyCode::Backspace => "bspace".to_string(),
            KeyCode::Insert => "ins".to_string(),
            KeyCode::Delete => "del".to_string(),
            KeyCode::PageUp => "pgup".to_string(),
            KeyCode::PageDown => "pgdn".to_string(),
            KeyCode::Up => "up".to_string(),
            KeyCode::Down => "down".to_string(),
            KeyCode::Left => "left".to_string(),
            KeyCode::Right => "right".to_string(),
            KeyCode::Home => "home".to_string(),
            KeyCode::End => "end".to_string(),
            _ => "".to_string(),
        };
        format!("{prefix}{scode}")
    }
}
