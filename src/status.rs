use std::borrow::Borrow;
use std::cmp::min;
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path;
use std::process::Command;

use regex::Regex;

use crate::args::Args;
use crate::completion::Completion;
use crate::config::Config;
use crate::file_window::{FilesWindow, WINDOW_MARGIN_TOP};
use crate::fileinfo::{FileKind, PathContent, SortBy};
use crate::input::Input;
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
    pub file_index: usize,
    /// The indexes of displayed file
    pub window: FilesWindow,
    /// Files marked as flagged
    pub flagged: HashSet<path::PathBuf>,
    /// String typed by the user in relevant modes
    pub input: Input,
    /// Files in current path
    pub path_content: PathContent,
    /// Height of the terminal window
    height: usize,
    /// Args readed from command line
    show_hidden: bool,
    /// Configurable terminal executable
    terminal: String,
    /// Configurable file opener. Default to "xgg-open"
    opener: String,
    /// Index in the jump list
    pub jump_index: usize,
    /// Completion list and index in it.
    pub completion: Completion,
    /// Last edition command kind received
    pub last_edition: LastEdition,
    must_quit: bool,
}

impl Status {
    /// Creates a new status from args, config and height.
    pub fn new(args: Args, config: Config, height: usize) -> Self {
        let path = std::fs::canonicalize(path::Path::new(&args.path)).unwrap_or_else(|_| {
            eprintln!("File does not exists {:?}", args.path);
            std::process::exit(2)
        });
        let path_content = PathContent::new(path, args.all);
        let show_hidden = args.all;
        let terminal = config.terminal;
        let opener = config.opener;
        let mode = Mode::Normal;
        let file_index = 0;
        let window = FilesWindow::new(path_content.files.len(), height);
        let flagged = HashSet::new();
        let input = Input::default();
        let jump_index = 0;
        let completion = Completion::default();
        let last_edition = LastEdition::Nothing;
        let must_quit = false;
        Self {
            mode,
            file_index,
            window,
            flagged,
            input,
            path_content,
            height,
            show_hidden,
            terminal,
            opener,
            jump_index,
            completion,
            last_edition,
            must_quit,
        }
    }

    pub fn event_normal(&mut self) {
        self.input.reset();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
        self.mode = Mode::Normal;
    }

    pub fn event_up_one_row(&mut self) {
        if self.file_index > 0 {
            self.file_index -= 1;
        }
        self.path_content.select_prev();
        self.window.scroll_up_one(self.file_index);
    }

    pub fn event_down_one_row(&mut self) {
        if self.file_index < self.path_content.files.len() - WINDOW_MARGIN_TOP {
            self.file_index += 1;
        }
        self.path_content.select_next();
        self.window.scroll_down_one(self.file_index);
    }

    pub fn event_go_top(&mut self) {
        self.path_content.select_index(0);
        self.file_index = 0;
        self.window.scroll_to(0);
    }

    pub fn event_up_10_rows(&mut self) {
        let up_index = if self.file_index > 10 {
            self.file_index - 10
        } else {
            0
        };
        self.path_content.select_index(up_index);
        self.file_index = up_index;
        self.window.scroll_to(up_index);
    }

    pub fn event_go_bottom(&mut self) {
        let last_index = self.path_content.files.len() - 1;
        self.path_content.select_index(last_index);
        self.file_index = last_index;
        self.window.scroll_to(last_index);
    }

    pub fn event_cursor_home(&mut self) {
        self.input.cursor_start()
    }

    pub fn event_cursor_end(&mut self) {
        self.input.cursor_end()
    }

    pub fn event_down_10_rows(&mut self) {
        let down_index = min(self.path_content.files.len() - 1, self.file_index + 10);
        self.path_content.select_index(down_index);
        self.file_index = down_index;
        self.window.scroll_to(down_index);
    }

    pub fn event_select_row(&mut self, row: u16) {
        self.file_index = (row - 1).into();
        self.path_content.select_index(self.file_index);
        self.window.scroll_to(self.file_index)
    }

    pub fn event_jumplist_next(&mut self) {
        if self.jump_index < self.flagged.len() {
            self.jump_index += 1;
        }
    }

    pub fn event_jumplist_prev(&mut self) {
        if self.jump_index > 0 {
            self.jump_index -= 1;
        }
    }

    pub fn event_move_to_parent(&mut self) {
        match self.path_content.path.parent() {
            Some(parent) => {
                self.path_content = PathContent::new(path::PathBuf::from(parent), self.show_hidden);
                self.window.reset(self.path_content.files.len());
                self.file_index = 0;
                self.input.cursor_start()
            }
            None => (),
        }
    }

    pub fn event_move_cursor_left(&mut self) {
        self.input.cursor_left()
    }

    pub fn event_go_to_child(&mut self) {
        if let FileKind::Directory = self.path_content.files[self.path_content.selected].file_kind {
            self.path_content = PathContent::new(
                self.path_content.files[self.path_content.selected]
                    .path
                    .clone(),
                self.show_hidden,
            );
            self.window.reset(self.path_content.files.len());
            self.file_index = 0;
            self.input.cursor_start()
        }
    }

    pub fn event_move_cursor_right(&mut self) {
        self.input.cursor_right()
    }

    pub fn event_delete_char_left(&mut self) {
        self.input.delete_char_left()
    }

    pub fn event_delete_chars_right(&mut self) {
        self.input.delete_chars_right()
    }

    pub fn event_text_insert_and_complete(&mut self, c: char) {
        self.event_text_insertion(c);
        self.fill_completion();
    }

    pub fn event_copy_paste(&mut self) {
        self.mode = Mode::NeedConfirmation;
        self.last_edition = LastEdition::CopyPaste;
    }

    pub fn event_cur_paste(&mut self) {
        self.mode = Mode::NeedConfirmation;
        self.last_edition = LastEdition::CutPaste;
    }

    pub fn event_new_dir(&mut self) {
        self.mode = Mode::Newdir
    }

    pub fn event_new_file(&mut self) {
        self.mode = Mode::Newfile
    }

    pub fn event_exec(&mut self) {
        self.mode = Mode::Exec
    }

    pub fn event_clear_flags(&mut self) {
        self.flagged.clear()
    }

    pub fn event_delete_file(&mut self) {
        self.mode = Mode::NeedConfirmation;
        self.last_edition = LastEdition::Delete;
    }

    pub fn event_help(&mut self) {
        self.mode = Mode::Help
    }

    pub fn event_search(&mut self) {
        self.mode = Mode::Search
    }

    pub fn event_regex_match(&mut self) {
        self.mode = Mode::RegexMatch
    }

    pub fn event_sort(&mut self) {
        self.mode = Mode::Sort;
    }

    pub fn event_quit(&mut self) {
        self.must_quit = true
    }

    pub fn event_leave_need_confirmation(&mut self) {
        self.last_edition = LastEdition::Nothing;
        self.mode = Mode::Normal;
    }

    pub fn event_leave_sort(&mut self, c: char) {
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
            self.event_go_top();
            self.path_content.select_index(0)
        }
    }

    pub fn event_text_insertion(&mut self, c: char) {
        self.input.insert(c);
    }

    pub fn event_toggle_flag(&mut self) {
        self.toggle_flag_on_path(self.path_content.files[self.file_index].path.clone());
        if self.file_index < self.path_content.files.len() - WINDOW_MARGIN_TOP {
            self.file_index += 1
        }
        self.path_content.select_next();
        self.window.scroll_down_one(self.file_index)
    }

    pub fn event_flag_all(&mut self) {
        self.path_content.files.iter().for_each(|file| {
            self.flagged.insert(file.path.clone());
        })
    }

    pub fn event_reverse_flags(&mut self) {
        // TODO: is there a way to use `toggle_flag_on_path` ? 2 mutable borrows...
        self.path_content.files.iter().for_each(|file| {
            if self.flagged.contains(&file.path.clone()) {
                self.flagged.remove(&file.path.clone());
            } else {
                self.flagged.insert(file.path.clone());
            }
        });
    }

    pub fn event_toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.path_content.show_hidden = !self.path_content.show_hidden;
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len())
    }

    pub fn event_open_file(&mut self) {
        execute_in_child(
            &self.opener,
            &vec![self.path_content.files[self.path_content.selected]
                .path
                .to_str()
                .unwrap()],
        );
    }

    pub fn event_rename(&mut self) {
        self.mode = Mode::Rename;
    }

    pub fn event_chmod(&mut self) {
        self.mode = Mode::Chmod;
        if self.flagged.is_empty() {
            self.flagged.insert(
                self.path_content.files[self.path_content.selected]
                    .path
                    .clone(),
            );
        }
    }

    pub fn event_goto(&mut self) {
        self.mode = Mode::Goto;
        self.completion.reset();
    }

    pub fn event_shell(&mut self) {
        execute_in_child(
            &self.terminal,
            &vec!["-d", self.path_content.path.to_str().unwrap()],
        );
    }

    pub fn event_jump(&mut self) {
        if !self.flagged.is_empty() {
            self.jump_index = 0;
            self.mode = Mode::Jump
        }
    }

    pub fn event_right_click(&mut self, row: u16) {
        self.file_index = (row - 1).into();
        self.path_content.select_index(self.file_index);
        self.window.scroll_to(self.file_index);
        if let FileKind::Directory = self.path_content.files[self.file_index].file_kind {
            self.event_go_to_child()
        } else {
            self.event_open_file()
        }
    }

    pub fn event_replace_input_with_completion(&mut self) {
        self.input.replace(self.completion.current_proposition())
    }

    pub fn event_nvim_filepicker(&mut self) {
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

    pub fn exec_last_edition(&mut self) {
        match self.last_edition {
            LastEdition::Delete => self.exec_delete_files(),
            LastEdition::CutPaste => self.exec_cut_paste(),
            LastEdition::CopyPaste => self.exec_copy_paste(),
            LastEdition::Nothing => (),
        }
        self.mode = Mode::Normal;
        self.last_edition = LastEdition::Nothing;
    }

    pub fn exec_rename(&mut self) {
        fs::rename(
            self.path_content.files[self.path_content.selected]
                .clone()
                .path,
            self.path_content
                .path
                .to_path_buf()
                .join(&self.input.string),
        )
        .unwrap_or(());
        self.refresh_view()
    }

    fn exec_delete_files(&mut self) {
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

    pub fn exec_newfile(&mut self) {
        if fs::File::create(self.path_content.path.join(self.input.string.clone())).is_ok() {}
        self.refresh_view()
    }

    pub fn exec_newdir(&mut self) {
        fs::create_dir(self.path_content.path.join(self.input.string.clone())).unwrap_or(());
        self.refresh_view()
    }

    pub fn exec_chmod(&mut self) {
        if self.input.string.is_empty() {
            return;
        }
        let permissions: u32 = u32::from_str_radix(&self.input.string, 8).unwrap_or(0_u32);
        if permissions <= MAX_PERMISSIONS {
            for path in self.flagged.iter() {
                Self::set_permissions(path.clone(), permissions).unwrap_or(())
            }
            self.flagged.clear()
        }
        self.input.string.clear();
        self.refresh_view()
    }

    pub fn exec_exec(&mut self) {
        let exec_command = self.input.string.clone();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        args.push(
            self.path_content.files[self.path_content.selected]
                .path
                .to_str()
                .unwrap(),
        );
        self.input.reset();
        execute_in_child(command, &args);
    }

    pub fn exec_search(&mut self) {
        let searched_term = self.input.string.clone();
        let mut next_index = self.file_index;
        for (index, file) in self.path_content.files.iter().enumerate().skip(next_index) {
            if file.filename.contains(&searched_term) {
                next_index = index;
                break;
            };
        }
        self.input.reset();
        self.path_content.select_index(next_index);
        self.file_index = next_index;
        self.window.scroll_to(self.file_index);
    }

    pub fn exec_goto(&mut self) {
        let target_string = self.input.string.clone();
        self.input.reset();
        let expanded_cow_path = shellexpand::tilde(&target_string);
        let expanded_target: &str = expanded_cow_path.borrow();
        if let Ok(path) = std::fs::canonicalize(expanded_target) {
            self.path_content = PathContent::new(path, self.show_hidden);
            self.window.reset(self.path_content.files.len());
        }
    }

    pub fn exec_jump(&mut self) {
        self.input.reset();
        let jump_list: Vec<&path::PathBuf> = self.flagged.iter().collect();
        let jump_target = jump_list[self.jump_index].clone();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent.to_path_buf(),
            None => jump_target.clone(),
        };
        self.path_content = PathContent::new(target_dir, self.show_hidden);
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

    pub fn exec_regex(&mut self) {
        let re = Regex::new(&self.input.string).unwrap();
        if !self.input.string.is_empty() {
            self.flagged.clear();
            for file in self.path_content.files.iter() {
                if re.is_match(file.path.to_str().unwrap()) {
                    self.flagged.insert(file.path.clone());
                }
            }
        }
        self.input.reset();
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
                                    && filename_startswith(e, &self.input.string)
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
                        .filter(|f| f.filename.contains(&self.input.string))
                        .map(|f| f.filename.clone())
                        .collect(),
                );
            }
            _ => (),
        }
    }

    fn refresh_view(&mut self) {
        self.file_index = 0;
        self.input.reset();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    /// Set the height of the window and itself.
    pub fn set_height(&mut self, height: usize) {
        self.window.set_height(height);
        self.height = height;
    }

    fn split_input_string(&self) -> (String, String) {
        let steps = self.input.string.split('/');
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

    pub fn must_quit(&self) -> bool {
        self.must_quit
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
