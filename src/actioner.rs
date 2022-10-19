use std::collections::HashMap;
use std::sync::Arc;
use tuikit::prelude::{Event, Key, MouseButton};
use tuikit::term::Term;

use crate::config::Keybindings;
use crate::event_char::EventChar;
use crate::mode::{MarkAction, Mode};
use crate::skim::Skimer;
use crate::tabs::Tabs;

/// Struct which mutates `tabs.selected()..
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on tabs.selected().
/// Keybindings are read from `Config`.
pub struct Actioner {
    binds: HashMap<char, EventChar>,
    term: Arc<Term>,
}

impl Actioner {
    /// Creates a map of configurable keybindings to `EventChar`
    /// The `EventChar` is then associated to a `tabs.selected(). method.
    pub fn new(keybindings: &Keybindings, term: Arc<Term>) -> Self {
        let binds = HashMap::from([
            (keybindings.toggle_hidden, EventChar::ToggleHidden),
            (keybindings.copy_paste, EventChar::CopyPaste),
            (keybindings.cut_paste, EventChar::CutPaste),
            (keybindings.newdir, EventChar::NewDir),
            (keybindings.newfile, EventChar::NewFile),
            (keybindings.chmod, EventChar::Chmod),
            (keybindings.exec, EventChar::Exec),
            (keybindings.goto, EventChar::Goto),
            (keybindings.rename, EventChar::Rename),
            (keybindings.clear_flags, EventChar::ClearFlags),
            (keybindings.toggle_flag, EventChar::ToggleFlag),
            (keybindings.shell, EventChar::Shell),
            (keybindings.delete, EventChar::DeleteFile),
            (keybindings.open_file, EventChar::OpenFile),
            (keybindings.help, EventChar::Help),
            (keybindings.search, EventChar::Search),
            (keybindings.regex_match, EventChar::RegexMatch),
            (keybindings.quit, EventChar::Quit),
            (keybindings.flag_all, EventChar::FlagAll),
            (keybindings.reverse_flags, EventChar::ReverseFlags),
            (keybindings.jump, EventChar::Jump),
            (keybindings.nvim, EventChar::NvimFilepicker),
            (keybindings.sort_by, EventChar::Sort),
            (keybindings.symlink, EventChar::Symlink),
            (keybindings.preview, EventChar::Preview),
            (keybindings.history, EventChar::History),
            (keybindings.shortcut, EventChar::Shortcut),
            (keybindings.bulkrename, EventChar::Bulkrename),
            (keybindings.marks_new, EventChar::MarksNew),
            (keybindings.marks_jump, EventChar::MarksJump),
        ]);
        Self { binds, term }
    }
    /// Reaction to received events.
    pub fn read_event(&self, tabs: &mut Tabs, ev: Event) {
        match ev {
            Event::Key(Key::ESC) => self.escape(tabs),
            Event::Key(Key::Up) => self.up(tabs),
            Event::Key(Key::Down) => self.down(tabs),
            Event::Key(Key::Left) => self.left(tabs),
            Event::Key(Key::Right) => self.right(tabs),
            Event::Key(Key::Backspace) => self.backspace(tabs),
            Event::Key(Key::Ctrl('d')) => self.delete(tabs),
            Event::Key(Key::Ctrl('q')) => self.escape(tabs),
            Event::Key(Key::Delete) => self.delete(tabs),
            Event::Key(Key::Insert) => self.insert(tabs),
            Event::Key(Key::Char(c)) => self.char(tabs, c),
            Event::Key(Key::Home) => self.home(tabs),
            Event::Key(Key::End) => self.end(tabs),
            Event::Key(Key::PageDown) => self.page_down(tabs),
            Event::Key(Key::PageUp) => self.page_up(tabs),
            Event::Key(Key::Enter) => self.enter(tabs),
            Event::Key(Key::Tab) => self.tab(tabs),
            Event::Key(Key::WheelUp(_, _, _)) => self.up(tabs),
            Event::Key(Key::WheelDown(_, _, _)) => self.down(tabs),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => self.left_click(tabs, row),
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => self.right_click(tabs, row),
            Event::Key(Key::Ctrl('f')) => self.ctrl_f(tabs),
            _ => {}
        }
    }

    /// Leaving a mode reset the window
    fn escape(&self, tabs: &mut Tabs) {
        tabs.selected().event_normal()
    }

    /// Move one line up
    fn up(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => tabs.selected().event_up_one_row(),
            Mode::Jump => tabs.event_jumplist_prev(),
            Mode::History => tabs.selected().event_history_prev(),
            Mode::Shortcut => tabs.selected().event_shortcut_prev(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                tabs.selected().completion.prev();
            }
            _ => (),
        }
    }

    /// Move one line down
    fn down(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => tabs.selected().event_down_one_row(),
            Mode::Jump => tabs.event_jumplist_next(),
            Mode::History => tabs.selected().event_history_next(),
            Mode::Shortcut => tabs.selected().event_shortcut_next(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                tabs.selected().completion.next();
            }
            _ => (),
        }
    }

    /// Move left in a string, move to parent in normal mode
    fn left(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal => tabs.selected().event_move_to_parent(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => tabs.selected().event_move_cursor_left(),
            _ => (),
        }
    }

    /// Move right in a string, move to children in normal mode.
    fn right(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal => tabs.selected().event_go_to_child(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => tabs.selected().event_move_cursor_right(),
            _ => (),
        }
    }

    /// Deletes a char in input string
    fn backspace(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => tabs.selected().event_delete_char_left(),
            Mode::Normal => (),
            _ => (),
        }
    }

    /// Deletes chars right of cursor in input string.
    /// Remove current tab in normal mode.
    fn delete(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => tabs.selected().event_delete_chars_right(),
            Mode::Normal => tabs.drop_tab(),
            _ => (),
        }
    }

    /// Insert a new tab in normal mode
    fn insert(&self, tabs: &mut Tabs) {
        if let Mode::Normal = tabs.selected().mode {
            tabs.new_tab()
        }
    }

    /// Move to top or beggining of line.
    fn home(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => tabs.selected().event_go_top(),
            _ => tabs.selected().event_cursor_home(),
        }
    }

    /// Move to end or end of line.
    fn end(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => tabs.selected().event_go_bottom(),
            _ => tabs.selected().event_cursor_end(),
        }
    }

    /// Move down 10 rows
    fn page_down(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => tabs.selected().event_page_down(),
            _ => (),
        }
    }

    /// Move up 10 rows
    fn page_up(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => tabs.selected().event_page_up(),
            _ => (),
        }
    }

    /// Execute a command
    fn enter(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Rename => tabs.selected().exec_rename(),
            Mode::Newfile => tabs.selected().exec_newfile(),
            Mode::Newdir => tabs.selected().exec_newdir(),
            Mode::Chmod => tabs.exec_chmod(),
            Mode::Exec => tabs.selected().exec_exec(),
            Mode::Search => tabs.selected().exec_search(),
            Mode::Goto => tabs.selected().exec_goto(),
            Mode::RegexMatch => tabs.exec_regex(),
            Mode::Jump => tabs.exec_jump(),
            Mode::History => tabs.selected().exec_history(),
            Mode::Shortcut => tabs.selected().exec_shortcut(),
            Mode::Normal
            | Mode::NeedConfirmation
            | Mode::Help
            | Mode::Sort
            | Mode::Preview
            | Mode::Marks(_) => (),
        }

        tabs.selected().input.reset();
        tabs.selected().mode = Mode::Normal;
    }

    /// Select this file
    fn left_click(&self, tabs: &mut Tabs, row: u16) {
        if let Mode::Normal = tabs.selected().mode {
            tabs.selected().event_select_row(row)
        }
    }

    /// Open a directory or a file
    fn right_click(&self, tabs: &mut Tabs, row: u16) {
        if let Mode::Normal = tabs.selected().mode {
            tabs.selected().event_right_click(row)
        }
    }

    /// Select next completion and insert it
    fn tab(&self, tabs: &mut Tabs) {
        match tabs.selected().mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                tabs.selected().event_replace_input_with_completion()
            }
            Mode::Normal => tabs.next(),
            _ => (),
        }
    }

    fn ctrl_f(&self, tabs: &mut Tabs) {
        let output = Skimer::new(self.term.clone()).no_source(tabs.selected_non_mut().path_str());
        let _ = self.term.clear();
        tabs.create_tabs_from_skim(output);
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(&self, tabs: &mut Tabs, c: char) {
        match tabs.selected().mode {
            Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename | Mode::RegexMatch => {
                tabs.selected().event_text_insertion(c)
            }
            Mode::Goto | Mode::Exec | Mode::Search => {
                tabs.selected().event_text_insert_and_complete(c)
            }
            Mode::Normal => match self.binds.get(&c) {
                Some(event_char) => event_char.match_char(tabs),
                None => (),
            },
            Mode::Help | Mode::Preview | Mode::Shortcut => tabs.selected().event_normal(),
            Mode::Jump => (),
            Mode::History => (),
            Mode::NeedConfirmation => {
                if c == 'y' {
                    tabs.exec_last_edition()
                }
                tabs.selected().event_leave_need_confirmation()
            }
            Mode::Marks(MarkAction::Jump) => tabs.exec_marks_jump(c),
            Mode::Marks(MarkAction::New) => tabs.exec_marks_new(c),
            Mode::Sort => tabs.selected().event_leave_sort(c),
        }
    }
}
