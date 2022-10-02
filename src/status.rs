use std::borrow::Borrow;
use std::cmp::min;
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path;
use std::process::Command;

use regex::Regex;
use tuikit::prelude::{Event, Key, MouseButton};

use crate::args::Args;
use crate::completion::Completion;
use crate::config::Config;
use crate::file_window::{FilesWindow, WINDOW_MARGIN_TOP};
use crate::fileinfo::{FileKind, PathContent, SortBy};
use crate::last_edition::LastEdition;
use crate::mode::Mode;

const MAX_PERMISSIONS: u32 = 0o777;

/// Holds every thing about the current status of the application.
/// Is responsible to execute commands depending on received events, mutating
/// the status of the application.
/// Every change on the application comes here.
pub struct Status {
    /// The mode the application is currenty in
    pub mode: Mode,
    /// The given index of a file.
    file_index: usize,
    /// The indexes of displayed file
    pub window: FilesWindow,
    /// oldpath before renaming
    oldpath: path::PathBuf,
    /// Files marked as flagged
    pub flagged: HashSet<path::PathBuf>,
    /// Currently typed input string
    pub input_string: String,
    /// Column of the cursor in the input string
    pub input_string_cursor_index: usize,
    /// Content of the file and index of selected one
    pub path_content: PathContent,
    /// Height of the terminal window
    height: usize,
    /// Args readed from command line
    args: Args,
    /// Configuration (colors, keybindings, terminal, opener) read from
    /// config file or hardcoded.
    pub config: Config,
    /// Index in the jump list
    pub jump_index: usize,
    /// Completion list and index in it.
    pub completion: Completion,
    /// Last edition command kind received
    pub last_edition: LastEdition,
}

impl Status {
    /// Creates a new status from args, config and height.
    pub fn new(args: Args, config: Config, height: usize) -> Self {
        let path = std::fs::canonicalize(path::Path::new(&args.path)).unwrap_or_else(|_| {
            eprintln!("File does not exists {:?}", args.path);
            std::process::exit(2)
        });
        let path_content = PathContent::new(path, args.all);

        let mode = Mode::Normal;
        let file_index = 0;
        let window = FilesWindow::new(path_content.files.len(), height);
        let oldpath = path::PathBuf::new();
        let flagged = HashSet::new();
        let input_string = "".to_string();
        let input_string_cursor_index = 0;
        let jump_index = 0;
        let completion = Completion::default();
        let last_edition = LastEdition::Nothing;
        Self {
            mode,
            file_index,
            window,
            oldpath,
            flagged,
            input_string,
            input_string_cursor_index,
            path_content,
            height,
            args,
            config,
            jump_index,
            completion,
            last_edition,
        }
    }

    /// Reaction to received events.
    pub fn read_event(&mut self, ev: Event) {
        match ev {
            Event::Key(Key::ESC) => self.action_escape(),
            Event::Key(Key::Up) => self.action_up(),
            Event::Key(Key::Down) => self.action_down(),
            Event::Key(Key::Left) => self.action_left(),
            Event::Key(Key::Right) => self.action_right(),
            Event::Key(Key::Backspace) => self.action_backspace(),
            Event::Key(Key::Ctrl('d')) => self.action_delete(),
            Event::Key(Key::Delete) => self.action_delete(),
            Event::Key(Key::Char(c)) => self.action_char(c),
            Event::Key(Key::Home) => self.action_home(),
            Event::Key(Key::End) => self.action_end(),
            Event::Key(Key::PageDown) => self.action_page_down(),
            Event::Key(Key::PageUp) => self.action_page_up(),
            Event::Key(Key::Enter) => self.action_enter(),
            Event::Key(Key::Tab) => self.action_tab(),
            Event::Key(Key::WheelUp(_, _, _)) => self.action_up(),
            Event::Key(Key::WheelDown(_, _, _)) => self.action_down(),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => self.action_left_click(row),
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                self.action_right_click(row)
            }
            _ => {}
        }
    }

    /// Leaving a mode reset the window
    fn action_escape(&mut self) {
        self.event_escape()
    }

    /// Move one line up
    fn action_up(&mut self) {
        match self.mode {
            Mode::Normal => self.event_up_one_row(),
            Mode::Jump => self.event_jumplist_prev(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                self.completion.prev();
            }
            _ => (),
        }
    }

    /// Move one line down
    fn action_down(&mut self) {
        match self.mode {
            Mode::Normal => self.event_down_one_row(),
            Mode::Jump => self.event_jumplist_next(),
            Mode::Goto | Mode::Exec | Mode::Search => {
                self.completion.next();
            }
            _ => (),
        }
    }

    /// Move left in a string, move to parent in normal mode
    fn action_left(&mut self) {
        match self.mode {
            Mode::Normal => self.event_move_to_parent(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => self.event_move_cursor_left(),
            _ => (),
        }
    }

    /// Move right in a string, move to children in normal mode.
    fn action_right(&mut self) {
        match self.mode {
            Mode::Normal => self.event_go_to_child(),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => self.event_move_cursor_right(),
            _ => (),
        }
    }

    /// Deletes a char in input string
    fn action_backspace(&mut self) {
        match self.mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => self.event_delete_char_left(),
            Mode::Normal => (),
            _ => (),
        }
    }

    fn action_delete(&mut self) {
        match self.mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto => self.event_delete_chars_right(),
            Mode::Normal => (),
            _ => (),
        }
    }

    fn action_home(&mut self) {
        if let Mode::Normal = self.mode {
            self.path_content.select_index(0);
            self.file_index = 0;
            self.window.scroll_to(0);
        } else {
            self.input_string_cursor_index = 0;
        }
    }

    fn action_end(&mut self) {
        if let Mode::Normal = self.mode {
            let last_index = self.path_content.files.len() - 1;
            self.path_content.select_index(last_index);
            self.file_index = last_index;
            self.window.scroll_to(last_index);
        } else {
            self.input_string_cursor_index = self.input_string.len();
        }
    }

    fn action_page_down(&mut self) {
        if let Mode::Normal = self.mode {
            let down_index = min(self.path_content.files.len() - 1, self.file_index + 10);
            self.path_content.select_index(down_index);
            self.file_index = down_index;
            self.window.scroll_to(down_index);
        }
    }

    fn action_page_up(&mut self) {
        if let Mode::Normal = self.mode {
            let up_index = if self.file_index > 10 {
                self.file_index - 10
            } else {
                0
            };
            self.path_content.select_index(up_index);
            self.file_index = up_index;
            self.window.scroll_to(up_index);
        }
    }

    fn action_enter(&mut self) {
        match self.mode {
            Mode::Rename => self.exec_rename(),
            Mode::Newfile => self.exec_newfile(),
            Mode::Newdir => self.exec_newdir(),
            Mode::Chmod => self.exec_chmod(),
            Mode::Exec => self.exec_exec(),
            Mode::Search => self.exec_search(),
            Mode::Goto => self.exec_goto(),
            Mode::RegexMatch => self.exec_regex(),
            Mode::Jump => self.exec_jump(),
            Mode::Normal | Mode::NeedConfirmation | Mode::Help | Mode::Sort => (),
        }

        self.input_string_cursor_index = 0;
        self.mode = Mode::Normal;
    }

    fn action_left_click(&mut self, row: u16) {
        if let Mode::Normal = self.mode {
            self.file_index = (row - 1).into();
            self.path_content.select_index(self.file_index);
            self.window.scroll_to(self.file_index)
        }
    }

    fn action_right_click(&mut self, row: u16) {
        if let Mode::Normal = self.mode {
            self.file_index = (row - 1).into();
            self.path_content.select_index(self.file_index);
            self.window.scroll_to(self.file_index)
        }
        if let FileKind::Directory = self.path_content.files[self.file_index].file_kind {
            self.action_right()
        } else {
            self.event_open_file()
        }
    }

    fn action_tab(&mut self) {
        match self.mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                self.input_string = self.completion.current_proposition()
            }
            _ => (),
        }
    }
    fn action_char(&mut self, c: char) {
        match self.mode {
            Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename | Mode::RegexMatch => {
                self.event_text_insertion(c)
            }
            Mode::Goto | Mode::Exec | Mode::Search => self.event_text_insert_and_complete(c),
            Mode::Normal => {
                if c == self.config.keybindings.toggle_hidden {
                    self.event_toggle_hidden()
                } else if c == self.config.keybindings.copy_paste {
                    self.event_copy_paste()
                } else if c == self.config.keybindings.cut_paste {
                    self.event_cur_paste()
                } else if c == self.config.keybindings.newdir {
                    self.event_new_dir()
                } else if c == self.config.keybindings.newfile {
                    self.event_new_file()
                } else if c == self.config.keybindings.chmod {
                    self.event_chmod()
                } else if c == self.config.keybindings.exec {
                    self.event_exec()
                } else if c == self.config.keybindings.goto {
                    self.event_goto()
                } else if c == self.config.keybindings.rename {
                    self.event_rename()
                } else if c == self.config.keybindings.clear_flags {
                    self.event_clear_flags()
                } else if c == self.config.keybindings.toggle_flag {
                    self.event_toggle_flag()
                } else if c == self.config.keybindings.shell {
                    self.event_shell()
                } else if c == self.config.keybindings.delete {
                    self.event_delete_file()
                } else if c == self.config.keybindings.open_file {
                    self.event_open_file()
                } else if c == self.config.keybindings.help {
                    self.event_help()
                } else if c == self.config.keybindings.search {
                    self.event_search()
                } else if c == self.config.keybindings.regex_match {
                    self.event_regex_match()
                } else if c == self.config.keybindings.quit {
                    self.event_quit()
                } else if c == self.config.keybindings.flag_all {
                    self.event_flag_all()
                } else if c == self.config.keybindings.reverse_flags {
                    self.event_reverse_flags()
                } else if c == self.config.keybindings.jump {
                    self.event_jump();
                } else if c == self.config.keybindings.nvim {
                    self.event_nvim_filepicker()
                } else if c == self.config.keybindings.sort_by {
                    self.event_sort()
                }
            }
            Mode::Help => self.event_normal(),
            Mode::Jump => (),
            Mode::NeedConfirmation => {
                if c == 'y' {
                    self.exec_last_edition()
                }
                self.event_leave_need_confirmation()
            }
            Mode::Sort => self.event_leave_sort(c),
        }
    }

    fn event_escape(&mut self) {
        self.input_string.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
        self.mode = Mode::Normal;
        self.input_string_cursor_index = 0;
    }

    fn event_up_one_row(&mut self) {
        if self.file_index > 0 {
            self.file_index -= 1;
        }
        self.path_content.select_prev();
        self.window.scroll_up_one(self.file_index);
    }

    fn event_down_one_row(&mut self) {
        if self.file_index < self.path_content.files.len() - WINDOW_MARGIN_TOP {
            self.file_index += 1;
        }
        self.path_content.select_next();
        self.window.scroll_down_one(self.file_index);
    }

    fn event_jumplist_next(&mut self) {
        if self.jump_index < self.flagged.len() {
            self.jump_index += 1;
        }
    }

    fn event_jumplist_prev(&mut self) {
        if self.jump_index > 0 {
            self.jump_index -= 1;
        }
    }

    fn event_move_to_parent(&mut self) {
        match self.path_content.path.parent() {
            Some(parent) => {
                self.path_content = PathContent::new(path::PathBuf::from(parent), self.args.all);
                self.window.reset(self.path_content.files.len());
                self.file_index = 0;
                self.input_string_cursor_index = 0;
            }
            None => (),
        }
    }

    fn event_move_cursor_left(&mut self) {
        if self.input_string_cursor_index > 0 {
            self.input_string_cursor_index -= 1
        }
    }

    fn event_go_to_child(&mut self) {
        if let FileKind::Directory = self.path_content.files[self.path_content.selected].file_kind {
            self.path_content = PathContent::new(
                self.path_content.files[self.path_content.selected]
                    .path
                    .clone(),
                self.args.all,
            );
            self.window.reset(self.path_content.files.len());
            self.file_index = 0;
            self.input_string_cursor_index = 0;
        }
    }

    fn event_move_cursor_right(&mut self) {
        if self.input_string_cursor_index < self.input_string.len() {
            self.input_string_cursor_index += 1
        }
    }

    fn event_delete_char_left(&mut self) {
        if self.input_string_cursor_index > 0 && !self.input_string.is_empty() {
            self.input_string.remove(self.input_string_cursor_index - 1);
            self.input_string_cursor_index -= 1;
        }
    }

    fn event_delete_chars_right(&mut self) {
        self.input_string = self
            .input_string
            .chars()
            .into_iter()
            .take(self.input_string_cursor_index)
            .collect();
    }

    fn event_text_insert_and_complete(&mut self, c: char) {
        self.event_text_insertion(c);
        self.fill_completion();
    }

    fn event_copy_paste(&mut self) {
        self.mode = Mode::NeedConfirmation;
        self.last_edition = LastEdition::CopyPaste;
    }

    fn event_cur_paste(&mut self) {
        self.mode = Mode::NeedConfirmation;
        self.last_edition = LastEdition::CutPaste;
    }

    fn event_new_dir(&mut self) {
        self.mode = Mode::Newdir
    }

    fn event_new_file(&mut self) {
        self.mode = Mode::Newfile
    }

    fn event_exec(&mut self) {
        self.mode = Mode::Exec
    }

    fn event_clear_flags(&mut self) {
        self.flagged.clear()
    }

    fn event_delete_file(&mut self) {
        self.mode = Mode::NeedConfirmation;
        self.last_edition = LastEdition::Delete;
    }

    fn event_help(&mut self) {
        self.mode = Mode::Help
    }

    fn event_search(&mut self) {
        self.mode = Mode::Search
    }

    fn event_regex_match(&mut self) {
        self.mode = Mode::RegexMatch
    }

    fn event_sort(&mut self) {
        self.mode = Mode::Sort;
    }

    fn event_normal(&mut self) {
        self.mode = Mode::Normal
    }

    fn event_quit(&mut self) {
        std::process::exit(0);
    }

    fn event_leave_need_confirmation(&mut self) {
        self.last_edition = LastEdition::Nothing;
        self.mode = Mode::Normal;
    }

    fn event_leave_sort(&mut self, c: char) {
        self.mode = Mode::Normal;
        match c {
            'n' => self.path_content.sort_by = SortBy::Filename,
            'd' => self.path_content.sort_by = SortBy::Date,
            's' => self.path_content.sort_by = SortBy::Size,
            'e' => self.path_content.sort_by = SortBy::Extension,
            _ => {
                return;
            }
        }
        if !self.path_content.files.is_empty() {
            self.path_content.files[self.file_index].unselect();
            self.path_content.sort();
            self.action_home();
            self.path_content.select_index(0);
        }
    }

    fn event_text_insertion(&mut self, c: char) {
        self.input_string.insert(self.input_string_cursor_index, c);
        self.input_string_cursor_index += 1;
    }

    fn event_toggle_flag(&mut self) {
        self.toggle_flag_on_path(self.path_content.files[self.file_index].path.clone());
        if self.file_index < self.path_content.files.len() - WINDOW_MARGIN_TOP {
            self.file_index += 1;
        }
        self.path_content.select_next();
        self.window.scroll_down_one(self.file_index);
    }

    fn event_flag_all(&mut self) {
        self.path_content.files.iter().for_each(|file| {
            self.flagged.insert(file.path.clone());
        });
    }

    fn event_reverse_flags(&mut self) {
        // TODO: is there a way to use `toggle_flag_on_path` ? 2 mutable borrows...
        self.path_content.files.iter().for_each(|file| {
            if self.flagged.contains(&file.path.clone()) {
                self.flagged.remove(&file.path.clone());
            } else {
                self.flagged.insert(file.path.clone());
            }
        });
    }

    fn event_toggle_hidden(&mut self) {
        self.args.all = !self.args.all;
        self.path_content.show_hidden = !self.path_content.show_hidden;
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len())
    }

    fn event_open_file(&mut self) {
        execute_in_child(
            &self.config.opener,
            &vec![self.path_content.files[self.path_content.selected]
                .path
                .to_str()
                .unwrap()],
        );
    }
    fn event_rename(&mut self) {
        self.mode = Mode::Rename;
        let oldname = self.path_content.files[self.path_content.selected]
            .filename
            .clone();
        self.oldpath = self.path_content.path.to_path_buf();
        self.oldpath.push(oldname);
    }

    fn event_chmod(&mut self) {
        self.mode = Mode::Chmod;
        if self.flagged.is_empty() {
            self.flagged.insert(
                self.path_content.files[self.path_content.selected]
                    .path
                    .clone(),
            );
        }
    }

    fn event_goto(&mut self) {
        self.mode = Mode::Goto;
        self.completion.reset();
    }

    fn event_shell(&mut self) {
        execute_in_child(
            &self.config.terminal,
            &vec!["-d", self.path_content.path.to_str().unwrap()],
        );
    }

    fn event_delete(&mut self) {
        self.flagged.iter().for_each(|pathbuf| {
            if pathbuf.is_dir() {
                fs::remove_dir_all(pathbuf).unwrap_or(());
            } else {
                fs::remove_file(pathbuf).unwrap_or(());
            }
        });
        self.flagged.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }
    fn event_jump(&mut self) {
        if !self.flagged.is_empty() {
            self.jump_index = 0;
            self.mode = Mode::Jump
        }
    }

    fn event_nvim_filepicker(&mut self) {
        // "nvim-send --remote-send '<esc>:e readme.md<cr>' --servername 127.0.0.1:8888"
        let server = std::env::var("NVIM_LISTEN_ADDRESS").unwrap_or_else(|_| "".to_owned());
        if server.is_empty() {
            return;
        }
        execute_in_child(
            "nvim-send",
            &vec![
                "--remote-send",
                &format!(
                    "<esc>:e {}<cr><esc>:close<cr>",
                    self.path_content.files[self.file_index]
                        .path
                        .clone()
                        .to_str()
                        .unwrap()
                ),
                "--servername",
                &server,
            ],
        );
    }
    fn exec_copy_paste(&mut self) {
        self.flagged.iter().for_each(|oldpath| {
            let newpath = self
                .path_content
                .path
                .clone()
                .join(oldpath.as_path().file_name().unwrap());
            fs::copy(oldpath, newpath).unwrap_or(0);
        });
        self.flagged.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    fn exec_cut_paste(&mut self) {
        self.flagged.iter().for_each(|oldpath| {
            let newpath = self
                .path_content
                .path
                .clone()
                .join(oldpath.as_path().file_name().unwrap());
            fs::rename(oldpath, newpath).unwrap_or(());
        });
        self.flagged.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    fn exec_last_edition(&mut self) {
        match self.last_edition {
            LastEdition::Delete => self.event_delete(),
            LastEdition::CutPaste => self.exec_cut_paste(),
            LastEdition::CopyPaste => self.exec_copy_paste(),
            LastEdition::Nothing => (),
        }
        self.mode = Mode::Normal;
        self.last_edition = LastEdition::Nothing;
    }

    fn exec_rename(&mut self) {
        fs::rename(
            self.oldpath.clone(),
            self.path_content
                .path
                .to_path_buf()
                .join(&self.input_string),
        )
        .unwrap_or(());
        self.refresh_view()
    }

    fn exec_newfile(&mut self) {
        if fs::File::create(self.path_content.path.join(self.input_string.clone())).is_ok() {}
        self.refresh_view()
    }

    fn exec_newdir(&mut self) {
        fs::create_dir(self.path_content.path.join(self.input_string.clone())).unwrap_or(());
        self.refresh_view()
    }

    fn exec_chmod(&mut self) {
        if self.input_string.is_empty() {
            return;
        }
        let permissions: u32 = u32::from_str_radix(&self.input_string, 8).unwrap_or(0_u32);
        if permissions <= MAX_PERMISSIONS {
            for path in self.flagged.iter() {
                Self::set_permissions(path.clone(), permissions).unwrap_or(())
            }
            self.flagged.clear()
        }
        self.input_string.clear();
        self.refresh_view()
    }

    fn exec_exec(&mut self) {
        let exec_command = self.input_string.clone();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        args.push(
            self.path_content.files[self.path_content.selected]
                .path
                .to_str()
                .unwrap(),
        );
        self.input_string.clear();
        execute_in_child(command, &args);
    }

    fn exec_search(&mut self) {
        let searched_term = self.input_string.clone();
        let mut next_index = self.file_index;
        for (index, file) in self.path_content.files.iter().enumerate().skip(next_index) {
            if file.filename.contains(&searched_term) {
                next_index = index;
                break;
            };
        }
        self.input_string.clear();
        self.path_content.select_index(next_index);
        self.file_index = next_index;
        self.window.scroll_to(self.file_index);
    }

    fn exec_goto(&mut self) {
        let target_string = self.input_string.clone();
        self.input_string.clear();
        let expanded_cow_path = shellexpand::tilde(&target_string);
        let expanded_target: &str = expanded_cow_path.borrow();
        if let Ok(path) = std::fs::canonicalize(expanded_target) {
            self.path_content = PathContent::new(path, self.args.all);
            self.window.reset(self.path_content.files.len());
        }
    }

    fn exec_jump(&mut self) {
        self.input_string.clear();
        let jump_list: Vec<&path::PathBuf> = self.flagged.iter().collect();
        let jump_target = jump_list[self.jump_index].clone();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent.to_path_buf(),
            None => jump_target.clone(),
        };
        self.path_content = PathContent::new(target_dir, self.args.all);
        self.file_index = self
            .path_content
            .files
            .iter()
            .position(|file| file.path == jump_target.clone())
            .unwrap_or(0);
        self.path_content.select_index(self.file_index);
        self.window.reset(self.path_content.files.len());
        self.window.scroll_to(self.file_index);
    }

    fn exec_regex(&mut self) {
        let re = Regex::new(&self.input_string).unwrap();
        if !self.input_string.is_empty() {
            self.flagged.clear();
            for file in self.path_content.files.iter() {
                if re.is_match(file.path.to_str().unwrap()) {
                    self.flagged.insert(file.path.clone());
                }
            }
        }
        self.input_string.clear();
    }

    fn set_permissions(path: path::PathBuf, permissions: u32) -> Result<(), std::io::Error> {
        fs::set_permissions(path, fs::Permissions::from_mode(permissions))
    }

    fn fill_completion(&mut self) {
        match self.mode {
            Mode::Goto => {
                let (parent, last_name) = self.split_input_string();
                if last_name.is_empty() {
                    return;
                }
                if let Ok(path) = std::fs::canonicalize(parent) {
                    if let Ok(entries) = fs::read_dir(path) {
                        self.completion.update(
                            entries
                                .filter_map(|e| e.ok())
                                .filter(|e| {
                                    e.file_type().unwrap().is_dir()
                                        && filename_startswith(e, &last_name)
                                })
                                .map(|e| e.path().to_string_lossy().into_owned())
                                .collect(),
                        )
                    }
                }
            }
            Mode::Exec => {
                let mut proposals: Vec<String> = vec![];
                for path in std::env::var_os("PATH")
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .split(':')
                {
                    if let Ok(entries) = fs::read_dir(path) {
                        let comp: Vec<String> = entries
                            .filter(|e| e.is_ok())
                            .map(|e| e.unwrap())
                            .filter(|e| {
                                e.file_type().unwrap().is_file()
                                    && filename_startswith(e, &self.input_string)
                            })
                            .map(|e| e.path().to_string_lossy().into_owned())
                            .collect();
                        proposals.extend(comp);
                    }
                }
                self.completion.update(proposals);
            }
            Mode::Search => {
                self.completion.update(
                    self.path_content
                        .files
                        .iter()
                        .filter(|f| f.filename.contains(&self.input_string))
                        .map(|f| f.filename.clone())
                        .collect(),
                );
            }
            _ => (),
        }
    }

    fn refresh_view(&mut self) {
        self.file_index = 0;
        self.input_string.clear();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    /// Set the height of the window and itself.
    pub fn set_height(&mut self, height: usize) {
        self.window.set_height(height);
        self.height = height;
    }

    fn split_input_string(&self) -> (String, String) {
        let steps = self.input_string.split('/');
        let mut vec_steps: Vec<&str> = steps.collect();
        let last_name = vec_steps.pop().unwrap_or("").to_owned();
        let parent = self.create_parent(vec_steps);
        (parent, last_name)
    }

    fn create_parent(&self, vec_steps: Vec<&str>) -> String {
        let mut parent = if vec_steps.is_empty() || vec_steps.len() == 1 && vec_steps[0] != "~" {
            "/".to_owned()
        } else {
            "".to_owned()
        };
        parent.push_str(&vec_steps.join("/"));
        shellexpand::tilde(&parent).to_string()
    }

    fn toggle_flag_on_path(&mut self, path: path::PathBuf) {
        if self.flagged.contains(&path) {
            self.flagged.remove(&path);
        } else {
            self.flagged.insert(path);
        }
    }
}

/// true if the filename starts with a pattern
fn filename_startswith(entry: &std::fs::DirEntry, pattern: &String) -> bool {
    entry
        .file_name()
        .to_string_lossy()
        .into_owned()
        .starts_with(pattern)
}

/// Execute the command in a fork.
fn execute_in_child(exe: &str, args: &Vec<&str>) -> std::process::Child {
    eprintln!("exec exe {}, args {:?}", exe, args);
    Command::new(exe).args(args).spawn().unwrap()
}
