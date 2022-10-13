use std::borrow::Borrow;
use std::cmp::min;
use std::fs;
use std::path;
use std::process::Command;

use crate::args::Args;
use crate::completion::Completion;
use crate::config::Config;
use crate::content_window::ContentWindow;
use crate::fileinfo::{FileKind, PathContent, SortBy};
use crate::input::Input;
use crate::last_edition::LastEdition;
use crate::mode::Mode;
use crate::preview::Preview;
use crate::shortcut::Shortcut;
use crate::visited::History;

/// Holds every thing about the current status of the application.
/// Is responsible to execute commands depending on received events, mutating
/// the status of the application.
/// Every change on the application comes here.
#[derive(Clone)]
pub struct Status {
    /// The mode the application is currenty in
    pub mode: Mode,
    /// The given index of a file.
    pub line_index: usize,
    /// The indexes of displayed file
    pub window: ContentWindow,
    /// Files marked as flagged
    pub input: Input,
    /// Files in current path
    pub path_content: PathContent,
    /// Height of the terminal window
    height: usize,
    /// read from command line
    pub show_hidden: bool,
    /// Configurable terminal executable
    terminal: String,
    /// Configurable file opener. Default to "xdg-open"
    opener: String,
    /// Completion list and index in it.
    pub completion: Completion,
    /// Last edition command kind received
    pub last_edition: LastEdition,
    must_quit: bool,
    /// Lines of the previewed files.
    /// Empty if not in preview mode.
    pub preview: Preview,
    /// Visited directories
    pub history: History,
    /// Predefined shortcuts
    pub shortcut: Shortcut,
}

impl Status {
    /// Creates a new status from args, config and height.
    pub fn new(args: Args, config: Config, height: usize) -> Self {
        let path = std::fs::canonicalize(path::Path::new(&args.path)).unwrap_or_else(|_| {
            eprintln!("File does not exists {:?}", args.path);
            std::process::exit(2)
        });
        let path_content = PathContent::new(path.clone(), args.all);
        let show_hidden = args.all;
        let terminal = config.terminal;
        let opener = config.opener;
        let mode = Mode::Normal;
        let line_index = 0;
        let window = ContentWindow::new(path_content.files.len(), height);
        let input = Input::default();
        let completion = Completion::default();
        let last_edition = LastEdition::Nothing;
        let must_quit = false;
        let preview = Preview::Empty;
        let mut history = History::default();
        history.push(&path);
        let shortcut = Shortcut::default();
        Self {
            mode,
            line_index,
            window,
            input,
            path_content,
            height,
            show_hidden,
            terminal,
            opener,
            completion,
            last_edition,
            must_quit,
            preview,
            history,
            shortcut,
        }
    }

    pub fn event_normal(&mut self) {
        self.input.reset();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
        self.mode = Mode::Normal;
        self.preview = Preview::empty()
    }

    pub fn event_up_one_row(&mut self) {
        if let Mode::Normal = self.mode {
            self.path_content.select_prev();
        }
        if self.line_index > 0 {
            self.line_index -= 1;
        }
        self.window.scroll_up_one(self.line_index);
    }

    pub fn event_down_one_row(&mut self) {
        let max_line = if let Mode::Normal = self.mode {
            self.path_content.select_next();
            self.path_content.files.len()
        } else {
            self.preview.len()
        };
        if self.line_index < max_line - ContentWindow::WINDOW_MARGIN_TOP {
            self.line_index += 1;
        }
        self.window.scroll_down_one(self.line_index);
    }

    pub fn event_go_top(&mut self) {
        if let Mode::Preview = self.mode {
            self.path_content.select_index(0);
        }
        self.line_index = 0;
        self.window.scroll_to(0);
    }

    pub fn event_page_up(&mut self) {
        let scroll_up: usize = if let Mode::Normal = self.mode {
            10
        } else {
            self.height
        };
        let up_index = if self.line_index > scroll_up {
            self.line_index - scroll_up
        } else {
            0
        };
        if let Mode::Normal = self.mode {
            self.path_content.select_index(up_index);
        }
        self.line_index = up_index;
        self.window.scroll_to(up_index);
    }

    pub fn event_go_bottom(&mut self) {
        let last_index: usize;
        if let Mode::Normal = self.mode {
            last_index = self.path_content.files.len() - 1;
            self.path_content.select_index(last_index);
        } else {
            last_index = self.preview.len() - 1;
        }
        self.line_index = last_index;
        self.window.scroll_to(last_index);
    }

    pub fn event_cursor_home(&mut self) {
        self.input.cursor_start()
    }

    pub fn event_cursor_end(&mut self) {
        self.input.cursor_end()
    }

    pub fn event_page_down(&mut self) {
        let down_index = if let Mode::Normal = self.mode {
            min(self.path_content.files.len() - 1, self.line_index + 10)
        } else {
            min(self.preview.len() - 1, self.line_index + 30)
        };
        self.path_content.select_index(down_index);
        self.line_index = down_index;
        self.window.scroll_to(down_index);
    }

    pub fn event_select_row(&mut self, row: u16) {
        self.line_index = (row - 1).into();
        self.path_content.select_index(self.line_index);
        self.window.scroll_to(self.line_index)
    }

    pub fn event_shortcut_next(&mut self) {
        self.shortcut.next()
    }

    pub fn event_shortcut_prev(&mut self) {
        self.shortcut.prev()
    }

    pub fn event_history_next(&mut self) {
        self.history.next()
    }

    pub fn event_history_prev(&mut self) {
        self.history.prev()
    }

    pub fn event_move_to_parent(&mut self) {
        match self.path_content.path.parent() {
            Some(parent) => {
                let path = path::PathBuf::from(parent);
                self.history.push(&path);
                self.path_content = PathContent::new(path, self.show_hidden);
                self.window.reset(self.path_content.files.len());
                self.line_index = 0;
                self.input.cursor_start()
            }
            None => (),
        }
    }

    pub fn event_move_cursor_left(&mut self) {
        self.input.cursor_left()
    }

    pub fn event_go_to_child(&mut self) {
        if self.path_content.files.is_empty() {
            return;
        }
        if let FileKind::Directory = self.path_content.files[self.path_content.selected].file_kind {
            let childpath = self.path_content.selected_file().unwrap().path.clone();
            self.history.push(&childpath);
            self.path_content = PathContent::new(childpath, self.show_hidden);
            self.window.reset(self.path_content.files.len());
            self.line_index = 0;
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

    pub fn event_preview(&mut self) {
        if !self.path_content.files.is_empty() {
            self.mode = Mode::Preview;
            self.preview = Preview::new(&self.path_content);
            self.window.reset(self.preview.len())
        }
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
            'r' => self.path_content.reverse = !self.path_content.reverse,
            _ => {
                return;
            }
        }
        if !self.path_content.files.is_empty() {
            self.path_content.files[self.line_index].unselect();
            self.path_content.sort();
            if self.path_content.reverse {
                self.path_content.files.reverse();
            }
            self.event_go_top();
            self.path_content.select_index(0)
        }
    }

    pub fn event_text_insertion(&mut self, c: char) {
        self.input.insert(c);
    }

    pub fn event_toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.path_content.show_hidden = !self.path_content.show_hidden;
        self.path_content.reset_files();
        self.line_index = 0;
        self.window.reset(self.path_content.files.len())
    }

    pub fn event_open_file(&mut self) {
        if self.path_content.files.is_empty() {
            return;
        }
        execute_in_child(
            &self.opener,
            &vec![self
                .path_content
                .selected_file()
                .unwrap()
                .path
                .to_str()
                .unwrap()],
        );
    }

    pub fn event_rename(&mut self) {
        self.mode = Mode::Rename;
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

    pub fn event_history(&mut self) {
        self.mode = Mode::History
    }

    pub fn event_shortcut(&mut self) {
        self.mode = Mode::Shortcut
    }

    pub fn event_right_click(&mut self, row: u16) {
        if self.path_content.files.is_empty() || row as usize > self.path_content.files.len() {
            return;
        }
        self.line_index = (row - 1).into();
        self.path_content.select_index(self.line_index);
        self.window.scroll_to(self.line_index);
        if let FileKind::Directory = self.path_content.selected_file().unwrap().file_kind {
            self.event_go_to_child()
        } else {
            self.event_open_file()
        }
    }

    pub fn event_replace_input_with_completion(&mut self) {
        self.input.replace(self.completion.current_proposition())
    }

    pub fn event_nvim_filepicker(&mut self) {
        if self.path_content.files.is_empty() {
            return;
        }
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
                    self.path_content
                        .selected_file()
                        .unwrap()
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

    pub fn exec_rename(&mut self) {
        if self.path_content.files.is_empty() {
            return;
        }
        fs::rename(
            self.path_content.selected_file().unwrap().clone().path,
            self.path_content
                .path
                .to_path_buf()
                .join(&self.input.string),
        )
        .unwrap_or(());
        self.refresh_view()
    }

    pub fn exec_newfile(&mut self) {
        if fs::File::create(self.path_content.path.join(self.input.string.clone())).is_ok() {}
        self.refresh_view()
    }

    pub fn exec_newdir(&mut self) {
        fs::create_dir(self.path_content.path.join(self.input.string.clone())).unwrap_or(());
        self.refresh_view()
    }

    pub fn exec_exec(&mut self) {
        if self.path_content.files.is_empty() {
            return;
        }
        let exec_command = self.input.string.clone();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        args.push(
            self.path_content
                .selected_file()
                .unwrap()
                .path
                .to_str()
                .unwrap(),
        );
        self.input.reset();
        execute_in_child(command, &args);
    }

    pub fn exec_search(&mut self) {
        let searched_term = self.input.string.clone();
        let mut next_index = self.line_index;
        for (index, file) in self.path_content.files.iter().enumerate().skip(next_index) {
            if file.filename.contains(&searched_term) {
                next_index = index;
                break;
            };
        }
        self.input.reset();
        self.path_content.select_index(next_index);
        self.line_index = next_index;
        self.window.scroll_to(self.line_index);
    }

    pub fn exec_goto(&mut self) {
        let target_string = self.input.string.clone();
        self.input.reset();
        let expanded_cow_path = shellexpand::tilde(&target_string);
        let expanded_target: &str = expanded_cow_path.borrow();
        if let Ok(path) = std::fs::canonicalize(expanded_target) {
            self.history.push(&path);
            self.path_content = PathContent::new(path, self.show_hidden);
            self.window.reset(self.path_content.files.len());
        }
    }

    pub fn set_pathcontent(&mut self, path: path::PathBuf) {
        self.history.push(&path);
        self.path_content = PathContent::new(path, self.show_hidden);
        self.window.reset(self.path_content.files.len());
    }

    pub fn exec_shortcut(&mut self) {
        self.input.reset();
        let path = self.shortcut.selected();
        self.history.push(&path);
        self.path_content = PathContent::new(path, self.show_hidden);
        self.event_normal();
    }

    pub fn exec_history(&mut self) {
        self.input.reset();
        if let Some(path) = self.history.selected() {
            self.path_content = PathContent::new(path, self.show_hidden);
        }
        self.history.drop_queue();
        self.event_normal();
    }

    fn fill_completion(&mut self) {
        match self.mode {
            Mode::Goto => self.completion.goto(&self.input.string),
            Mode::Exec => self.completion.exec(&self.input.string),
            Mode::Search => self
                .completion
                .search(&self.input.string, &self.path_content),
            _ => (),
        }
    }

    pub fn refresh_view(&mut self) {
        self.line_index = 0;
        self.input.reset();
        self.path_content.reset_files();
        self.window.reset(self.path_content.files.len());
    }

    /// Set the height of the window and itself.
    pub fn set_height(&mut self, height: usize) {
        self.window.set_height(height);
        self.height = height;
    }

    /// Returns `true` iff the application has to quit.
    /// This methods allows use to reset the cursors and other
    /// terminal parameters gracefully.
    pub fn must_quit(&self) -> bool {
        self.must_quit
    }

    pub fn path_str(&self) -> String {
        self.path_content.path.to_str().unwrap().to_owned()
    }
}

// TODO: use [wait with output](https://doc.rust-lang.org/std/process/struct.Child.html#method.wait_with_output)
/// Execute the command in a fork.
fn execute_in_child(exe: &str, args: &Vec<&str>) -> std::process::Child {
    eprintln!("exec exe {}, args {:?}", exe, args);
    Command::new(exe).args(args).spawn().unwrap()
}
