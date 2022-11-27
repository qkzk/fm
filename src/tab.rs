use std::borrow::Borrow;
use std::cmp::min;
use std::fs;
use std::path;

use copypasta::{ClipboardContext, ClipboardProvider};
use log::info;

use crate::args::Args;
use crate::completion::Completion;
use crate::compress::decompress;
use crate::config::Config;
use crate::content_window::ContentWindow;
use crate::fileinfo::{FileKind, PathContent, SortBy};
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::input::Input;
use crate::last_edition::LastEdition;
use crate::mode::Mode;
use crate::opener::{execute_in_child, load_opener, Opener};
use crate::preview::Preview;
use crate::shortcut::Shortcut;
use crate::visited::History;

static OPENER_PATH: &str = "~/.config/fm/opener.yaml";

/// Holds every thing about the current tab of the application.
/// Is responsible to execute commands depending on received events, mutating
/// the tab of the application.
#[derive(Clone)]
pub struct Tab {
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
    /// NVIM RPC server address
    nvim_server: String,
    /// Configurable terminal executable
    terminal: String,
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
    pub opener: Opener,
}

impl Tab {
    /// Creates a new tab from args, config and height.
    pub fn new(args: Args, config: Config, height: usize) -> FmResult<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let path_content = PathContent::new(path.clone(), args.all)?;
        let show_hidden = args.all;
        let nvim_server = args.server;
        let terminal = config.terminal;
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
        let opener = load_opener(OPENER_PATH, terminal.clone())
            .unwrap_or_else(|_| Opener::new(terminal.clone()));
        Ok(Self {
            mode,
            line_index,
            window,
            input,
            path_content,
            height,
            show_hidden,
            nvim_server,
            terminal,
            completion,
            last_edition,
            must_quit,
            preview,
            history,
            shortcut,
            opener,
        })
    }

    pub fn event_normal(&mut self) -> FmResult<()> {
        self.input.reset();
        self.completion.reset();
        self.path_content.reset_files()?;
        self.window.reset(self.path_content.files.len());
        self.mode = Mode::Normal;
        self.preview = Preview::empty();
        Ok(())
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
        if max_line >= ContentWindow::WINDOW_MARGIN_TOP
            && self.line_index < max_line - ContentWindow::WINDOW_MARGIN_TOP
        {
            self.line_index += 1;
        }
        self.window.scroll_down_one(self.line_index);
    }

    pub fn event_go_top(&mut self) {
        if let Mode::Normal = self.mode {
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
        let down_index: usize;
        if let Mode::Normal = self.mode {
            down_index = min(self.path_content.files.len() - 1, self.line_index + 10);
            self.path_content.select_index(down_index);
        } else {
            down_index = min(self.preview.len() - 1, self.line_index + 30)
        }
        self.line_index = down_index;
        self.window.scroll_to(down_index);
    }

    pub fn event_select_row(&mut self, row: u16) {
        self.line_index = (row - 2).into();
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

    pub fn event_move_to_parent(&mut self) -> FmResult<()> {
        let parent = self.path_content.path.parent();
        let path = path::PathBuf::from(
            parent.ok_or_else(|| FmError::new("Root directory has no parent"))?,
        );
        self.history.push(&path);
        self.path_content = PathContent::new(path, self.show_hidden)?;
        self.window.reset(self.path_content.files.len());
        self.line_index = 0;
        self.input.cursor_start();
        Ok(())
    }

    pub fn event_move_cursor_left(&mut self) {
        self.input.cursor_left()
    }

    pub fn exec_file(&mut self) -> FmResult<()> {
        if self.path_content.is_empty() {
            return Ok(());
        }
        if self.path_content.is_selected_dir()? {
            self.go_to_child()
        } else {
            self.event_open_file()
        }
    }

    fn go_to_child(&mut self) -> FmResult<()> {
        let childpath = self
            .path_content
            .selected_file()
            .ok_or_else(|| FmError::new("Empty directory"))?
            .path
            .clone();
        self.history.push(&childpath);
        self.path_content = PathContent::new(childpath, self.show_hidden)?;
        self.window.reset(self.path_content.files.len());
        self.line_index = 0;
        self.input.cursor_start();
        Ok(())
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

    pub fn event_text_insert_and_complete(&mut self, c: char) -> FmResult<()> {
        self.event_text_insertion(c);
        self.fill_completion()
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

    pub fn event_preview(&mut self) -> FmResult<()> {
        if self.path_content.files.is_empty() {
            return Err(FmError::new("No file to preview"));
        };
        self.mode = Mode::Preview;
        self.preview = Preview::new(&self.path_content)?;
        self.window.reset(self.preview.len());
        Ok(())
    }

    pub fn event_delete_file(&mut self) {
        self.mode = Mode::NeedConfirmation;
        self.last_edition = LastEdition::Delete;
    }

    pub fn event_help(&mut self) {
        self.mode = Mode::Help;
        self.preview = Preview::help();
        self.window.reset(self.preview.len())
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
            'k' => self.path_content.sort_by = SortBy::Kind,
            'n' => self.path_content.sort_by = SortBy::Filename,
            'm' => self.path_content.sort_by = SortBy::Date,
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

    pub fn event_toggle_hidden(&mut self) -> FmResult<()> {
        self.show_hidden = !self.show_hidden;
        self.path_content.show_hidden = !self.path_content.show_hidden;
        self.path_content.reset_files()?;
        self.line_index = 0;
        self.window.reset(self.path_content.files.len());
        Ok(())
    }

    pub fn event_open_file(&mut self) -> FmResult<()> {
        match self.opener.open(
            self.path_content
                .selected_file()
                .ok_or_else(|| FmError::new("Empty directory"))?
                .path
                .clone(),
        ) {
            Ok(_) => (),
            Err(e) => info!(
                "Error opening {:?}: {:?}",
                self.path_content.selected_file(),
                e
            ),
        }
        Ok(())
    }

    pub fn event_rename(&mut self) {
        self.mode = Mode::Rename;
    }

    pub fn event_goto(&mut self) {
        self.mode = Mode::Goto;
        self.completion.reset();
    }

    pub fn event_shell(&mut self) -> FmResult<()> {
        execute_in_child(
            &self.terminal,
            &vec![
                "-d",
                self.path_content
                    .path
                    .to_str()
                    .ok_or_else(|| FmError::new("Couldn't parse the path name"))?,
            ],
        )?;
        Ok(())
    }

    pub fn event_history(&mut self) {
        self.mode = Mode::History
    }

    pub fn event_shortcut(&mut self) {
        self.mode = Mode::Shortcut
    }

    pub fn event_right_click(&mut self, row: u16) -> FmResult<()> {
        if self.path_content.files.is_empty() || row as usize > self.path_content.files.len() + 1 {
            return Err(FmError::new("not found"));
        }
        self.line_index = (row - 2).into();
        self.path_content.select_index(self.line_index);
        self.window.scroll_to(self.line_index);
        if let FileKind::Directory = self
            .path_content
            .selected_file()
            .ok_or_else(|| FmError::new("not found"))?
            .file_kind
        {
            self.exec_file()
        } else {
            self.event_open_file()
        }
    }

    pub fn event_replace_input_with_completion(&mut self) {
        self.input.replace(self.completion.current_proposition())
    }

    pub fn event_nvim_filepicker(&mut self) {
        if self.path_content.files.is_empty() {
            info!("Called nvim filepicker in an empty directory.");
            return;
        }
        // "nvim-send --remote-send '<esc>:e readme.md<cr>' --servername 127.0.0.1:8888"
        if let Ok(nvim_listen_address) = self.nvim_listen_address() {
            if let Some(path_str) = self.path_content.selected_path_str() {
                let _ = execute_in_child(
                    "nvim-send",
                    &vec![
                        "--remote-send",
                        &format!("<esc>:e {}<cr><esc>:close<cr>", path_str),
                        "--servername",
                        &nvim_listen_address,
                    ],
                );
            }
        } else {
            info!("Nvim server not defined");
        }
    }

    pub fn event_filename_to_clipboard(&self) -> FmResult<()> {
        if let Some(file) = self.path_content.selected_file() {
            let filename = file.filename.clone();
            let mut ctx = ClipboardContext::new()?;
            ctx.set_contents(filename)?;
            // For some reason, it's not writen if you don't read it back...
            let _ = ctx.get_contents();
        }
        Ok(())
    }

    pub fn event_filepath_to_clipboard(&self) -> FmResult<()> {
        if let Some(filepath) = self.path_content.selected_path_str() {
            let mut ctx = ClipboardContext::new()?;
            ctx.set_contents(filepath)?;
            // For some reason, it's not writen if you don't read it back...
            let _ = ctx.get_contents();
        }
        Ok(())
    }

    pub fn event_filter(&mut self) -> FmResult<()> {
        self.mode = Mode::Filter;
        Ok(())
    }

    pub fn event_decompress(&mut self) -> FmResult<()> {
        if let Some(fileinfo) = self.path_content.selected_file() {
            decompress(&fileinfo.path)
        } else {
            Ok(())
        }
    }

    pub fn event_back(&mut self) -> FmResult<()> {
        eprintln!("event back");
        if self.history.visited.len() <= 1 {
            return Ok(());
        }
        self.history.visited.pop();
        let last = self.history.visited[self.history.len() - 1].clone();
        eprintln!("moving back to {:?}", last);
        self.set_pathcontent(last)?;

        Ok(())
    }

    fn nvim_listen_address(&self) -> Result<String, std::env::VarError> {
        if !self.nvim_server.is_empty() {
            Ok(self.nvim_server.clone())
        } else {
            std::env::var("NVIM_LISTEN_ADDRESS")
        }
    }

    pub fn exec_rename(&mut self) -> FmResult<()> {
        if self.path_content.files.is_empty() {
            return Err(FmError::new("Empty directory"));
        }
        fs::rename(
            self.path_content
                .selected_path_str()
                .ok_or_else(|| FmError::new("File not found"))?,
            self.path_content
                .path
                .to_path_buf()
                .join(&self.input.string),
        )?;
        self.refresh_view()
    }

    pub fn exec_newfile(&mut self) -> FmResult<()> {
        fs::File::create(self.path_content.path.join(self.input.string.clone()))?;
        self.refresh_view()
    }

    pub fn exec_newdir(&mut self) -> FmResult<()> {
        match fs::create_dir(self.path_content.path.join(self.input.string.clone())) {
            Ok(()) => (),
            Err(e) => match e.kind() {
                std::io::ErrorKind::AlreadyExists => (),
                _ => return Err(FmError::from(e)),
            },
        }
        self.refresh_view()
    }

    pub fn exec_exec(&mut self) -> FmResult<()> {
        if self.path_content.files.is_empty() {
            return Err(FmError::new("empty directory"));
        }
        let exec_command = self.input.string.clone();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        if std::path::Path::new(command).exists() {
            let path = &self
                .path_content
                .selected_path_str()
                .ok_or_else(|| FmError::new("path unreachable"))?;
            args.push(path);
            execute_in_child(command, &args)?;
            self.completion.reset();
            self.input.reset();
        }
        Ok(())
    }

    pub fn event_drag_n_drop(&mut self) -> FmResult<()> {
        execute_in_child(
            "dragon-drop",
            &vec![&self
                .path_content
                .selected_path_str()
                .ok_or_else(|| FmError::new("path unreachable"))?],
        )?;
        Ok(())
    }

    pub fn exec_search(&mut self) {
        self.input.reset();
        let completed = self.completion.current_proposition();
        if completed.is_empty() {
            return;
        }
        let mut next_index = self.line_index;
        for (index, file) in self.path_content.files.iter().enumerate().skip(next_index) {
            if file.filename == completed {
                next_index = index;
                break;
            };
        }
        self.path_content.select_index(next_index);
        self.line_index = next_index;
        self.window.scroll_to(self.line_index);
    }

    pub fn exec_goto(&mut self) -> FmResult<()> {
        let target_string = self.input.string.clone();
        self.input.reset();
        let expanded_cow_path = shellexpand::tilde(&target_string);
        let expanded_target: &str = expanded_cow_path.borrow();
        let path = std::fs::canonicalize(expanded_target)?;
        self.history.push(&path);
        self.path_content = PathContent::new(path, self.show_hidden)?;
        self.window.reset(self.path_content.files.len());
        Ok(())
    }

    pub fn set_pathcontent(&mut self, path: path::PathBuf) -> FmResult<()> {
        self.history.push(&path);
        self.path_content = PathContent::new(path, self.show_hidden)?;
        self.window.reset(self.path_content.files.len());
        Ok(())
    }

    pub fn exec_shortcut(&mut self) -> FmResult<()> {
        self.input.reset();
        let path = self.shortcut.selected();
        self.history.push(&path);
        self.path_content = PathContent::new(path, self.show_hidden)?;
        self.event_normal()
    }

    pub fn exec_history(&mut self) -> FmResult<()> {
        self.input.reset();
        self.path_content = PathContent::new(
            self.history
                .selected()
                .ok_or_else(|| FmError::new("path unreachable"))?,
            self.show_hidden,
        )?;
        self.history.drop_queue();
        self.event_normal()
    }

    pub fn exec_filter(&mut self) -> FmResult<()> {
        let filter = FilterKind::from_input(&self.input.string);
        self.path_content.set_filter(filter);
        self.input.reset();
        self.path_content.reset_files()?;
        self.event_normal()
    }

    fn fill_completion(&mut self) -> FmResult<()> {
        match self.mode {
            Mode::Goto => self.completion.goto(&self.input.string),
            Mode::Exec => self.completion.exec(&self.input.string),
            Mode::Search => self
                .completion
                .search(&self.input.string, &self.path_content),
            _ => Ok(()),
        }
    }

    pub fn refresh_view(&mut self) -> FmResult<()> {
        self.line_index = 0;
        self.input.reset();
        self.path_content.reset_files()?;
        self.window.reset(self.path_content.files.len());
        Ok(())
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

    pub fn path_str(&self) -> Option<String> {
        Some(self.path_content.path.to_str()?.to_owned())
    }
}
