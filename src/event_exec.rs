use std::borrow::Borrow;
use std::cmp::min;
use std::fs;
use std::path;
use std::path::PathBuf;

use crate::bulkrename::Bulkrename;
use crate::completion::CompletionKind;
use crate::constant_strings_paths::DEFAULT_DRAGNDROP;
use crate::content_window::{ContentWindow, RESERVED_ROWS};
use crate::copy_move::CopyMove;
use crate::fileinfo::{FileKind, PathContent, SortBy};
use crate::filter::FilterKind;
use crate::fm_error::{ErrorVariant, FmError, FmResult};
use crate::mode::{ConfirmedAction, InputKind, MarkAction, Mode};
use crate::opener::execute_in_child;
use crate::preview::Preview;
use crate::status::Status;
use crate::tab::Tab;
use crate::term_manager::MIN_WIDTH_FOR_DUAL_PANE;

use copypasta::{ClipboardContext, ClipboardProvider};
use log::info;

/// Every kind of mutation of the application is defined here.
/// It mutates `Status` or its children `tab`.
pub struct EventExec {}

impl EventExec {
    /// Reset the selected tab view to the default.
    pub fn refresh_selected_view(status: &mut Status) -> FmResult<()> {
        status.selected().refresh_view()
    }

    /// When a rezise event occurs, we may hide the second panel if the width
    /// isn't sufficiant to display enough information.
    /// We also need to know the new height of the terminal to start scrolling
    /// up or down.
    pub fn resize(status: &mut Status, width: usize, height: usize) -> FmResult<()> {
        if width < MIN_WIDTH_FOR_DUAL_PANE {
            status.select_tab(0)?;
            status.set_dual_pane(false);
        } else {
            status.set_dual_pane(true);
        }
        status.selected().set_height(height);
        Self::refresh_selected_view(status)?;
        Ok(())
    }

    /// Remove every flag on files in this directory and others.
    pub fn event_clear_flags(status: &mut Status) -> FmResult<()> {
        status.flagged.clear();
        Ok(())
    }

    /// Flag all files in the current directory.
    pub fn event_flag_all(status: &mut Status) -> FmResult<()> {
        status.tabs[status.index]
            .path_content
            .files
            .iter()
            .for_each(|file| {
                status.flagged.insert(file.path.clone());
            });
        status.reset_tabs_view()
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn event_reverse_flags(status: &mut Status) -> FmResult<()> {
        status.tabs[status.index]
            .path_content
            .files
            .iter()
            .for_each(|file| {
                if status.flagged.contains(&file.path.clone()) {
                    status.flagged.remove(&file.path.clone());
                } else {
                    status.flagged.insert(file.path.clone());
                }
            });
        status.reset_tabs_view()
    }

    /// Toggle a single flag and move down one row.
    pub fn event_toggle_flag(status: &mut Status) -> FmResult<()> {
        let file = status.tabs[status.index]
            .path_content
            .selected_file()
            .ok_or_else(|| {
                FmError::new(
                    ErrorVariant::CUSTOM("event toggle flag".to_owned()),
                    "No selected file",
                )
            })?;
        status.toggle_flag_on_path(file.path.clone());
        Self::event_down_one_row(status.selected());
        Ok(())
    }

    /// Move to the next file in the jump list.
    pub fn event_jumplist_next(status: &mut Status) {
        if status.jump_index < status.flagged.len() {
            status.jump_index += 1;
        }
    }

    /// Move to the previous file in the jump list.
    pub fn event_jumplist_prev(status: &mut Status) {
        if status.jump_index > 0 {
            status.jump_index -= 1;
        }
    }

    /// Change to CHMOD mode allowing to edit permissions of a file.
    pub fn event_chmod(status: &mut Status) -> FmResult<()> {
        if status.selected().path_content.files.is_empty() {
            return Ok(());
        }
        status.selected().mode = Mode::ReadInput(InputKind::Chmod);
        if status.flagged.is_empty() {
            status.flagged.insert(
                status.tabs[status.index]
                    .path_content
                    .selected_file()
                    .unwrap()
                    .path
                    .clone(),
            );
        };
        status.reset_tabs_view()
    }

    /// Enter JUMP mode, allowing to jump to any flagged file.
    /// Does nothing if no file is flagged.
    pub fn event_jump(status: &mut Status) -> FmResult<()> {
        if !status.flagged.is_empty() {
            status.jump_index = 0;
            status.selected().mode = Mode::Jump
        }
        Ok(())
    }

    /// Enter Marks new mode, allowing to bind a char to a path.
    pub fn event_marks_new(status: &mut Status) -> FmResult<()> {
        status.selected().mode = Mode::ReadInput(InputKind::Marks(MarkAction::New));
        Ok(())
    }

    /// Enter Marks jump mode, allowing to jump to a marked file.
    pub fn event_marks_jump(status: &mut Status) -> FmResult<()> {
        status.selected().mode = Mode::ReadInput(InputKind::Marks(MarkAction::Jump));
        Ok(())
    }

    /// Execute a new mark, saving it to a config file for futher use.
    pub fn exec_marks_new(status: &mut Status, c: char) -> FmResult<()> {
        let path = status.selected().path_content.path.clone();
        status.marks.new_mark(c, path)?;
        Self::event_normal(status.selected())
    }

    /// Execute a jump to a mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    pub fn exec_marks_jump(status: &mut Status, c: char) -> FmResult<()> {
        if let Some(path) = status.marks.get(c) {
            let path = path.to_owned();
            status.selected().history.push(&path);
            status.selected().path_content = PathContent::new(path, status.selected().show_hidden)?;
        };
        Self::event_normal(status.selected())
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn event_symlink(status: &mut Status) -> FmResult<()> {
        for oldpath in status.flagged.iter() {
            let newpath = status.tabs[status.index].path_content.path.clone().join(
                oldpath.as_path().file_name().ok_or_else(|| {
                    FmError::new(
                        ErrorVariant::CUSTOM("event symlink".to_owned()),
                        "File not found",
                    )
                })?,
            );
            std::os::unix::fs::symlink(oldpath, newpath)?;
        }
        status.clear_flags_and_reset_view()
    }

    /// Enter bulkrename mode, opening a random temp file where the user
    /// can edit the selected filenames.
    /// Once the temp file is saved, those file names are changed.
    pub fn event_bulkrename(status: &mut Status) -> FmResult<()> {
        Bulkrename::new(status.filtered_flagged_files())?.rename(&status.opener)?;
        status.selected().refresh_view()
    }

    /// Copy the flagged file to current directory.
    /// A progress bar is displayed and a notification is sent once it's done.
    pub fn exec_copy_paste(status: &mut Status) -> FmResult<()> {
        status.cut_or_copy_flagged_files(CopyMove::Copy)
    }

    /// Move the flagged file to current directory.
    /// A progress bar is displayed and a notification is sent once it's done.
    pub fn exec_cut_paste(status: &mut Status) -> FmResult<()> {
        status.cut_or_copy_flagged_files(CopyMove::Move)
    }

    /// Recursively delete all flagged files.
    pub fn exec_delete_files(status: &mut Status) -> FmResult<()> {
        for pathbuf in status.flagged.iter() {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(pathbuf)?;
            } else {
                std::fs::remove_file(pathbuf)?;
            }
        }
        status.clear_flags_and_reset_view()
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    pub fn exec_chmod(status: &mut Status) -> FmResult<()> {
        if status.selected().input.is_empty() {
            return Ok(());
        }
        let permissions: u32 =
            u32::from_str_radix(&status.selected().input.string(), 8).unwrap_or(0_u32);
        if permissions <= Status::MAX_PERMISSIONS {
            for path in status.flagged.iter() {
                Status::set_permissions(path.clone(), permissions)?
            }
            status.flagged.clear()
        }
        status.selected().refresh_view()?;
        status.reset_tabs_view()
    }

    fn _exec_confirmed_action(
        status: &mut Status,
        confirmed_action: ConfirmedAction,
    ) -> FmResult<()> {
        match confirmed_action {
            ConfirmedAction::Delete => Self::exec_delete_files(status),
            ConfirmedAction::Move => Self::exec_cut_paste(status),
            ConfirmedAction::Copy => Self::exec_copy_paste(status),
        }
    }

    /// Execute a jump to the selected flagged file.
    /// If the user selected a directory, we jump inside it.
    /// Otherwise, we jump to the parent and select the file.
    pub fn exec_jump(status: &mut Status) -> FmResult<()> {
        status.selected().input.clear();
        let jump_list: Vec<&PathBuf> = status.flagged.iter().collect();
        let jump_target = jump_list[status.jump_index].clone();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent.to_path_buf(),
            None => jump_target.clone(),
        };
        status.selected().history.push(&target_dir);
        status.selected().path_content =
            PathContent::new(target_dir, status.selected().show_hidden)?;
        if let Some(index) = status.find_jump_target(&jump_target) {
            status.selected().line_index = index;
        } else {
            status.selected().line_index = 0;
        }

        let s_index = status.tabs[status.index].line_index;
        status.tabs[status.index].path_content.select_index(s_index);
        let len = status.tabs[status.index].path_content.files.len();
        status.selected().window.reset(len);
        status.selected().window.scroll_to(s_index);
        Ok(())
    }

    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    pub fn exec_confirmed_action(
        status: &mut Status,
        confirmed_action: ConfirmedAction,
    ) -> FmResult<()> {
        Self::_exec_confirmed_action(status, confirmed_action)?;
        status.selected().mode = Mode::Normal;
        Ok(())
    }

    /// Select the first file matching the typed regex in current dir.
    pub fn exec_regex(status: &mut Status) -> Result<(), regex::Error> {
        status.select_from_regex()?;
        status.selected().input.reset();
        Ok(())
    }

    /// Leave current mode to normal mode.
    /// Reset the inputs and completion, reset the window, exit the preview.
    pub fn event_normal(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        tab.completion.reset();
        tab.path_content.reset_files()?;
        tab.window.reset(tab.path_content.files.len());
        tab.mode = Mode::Normal;
        tab.preview = Preview::empty();
        Ok(())
    }

    /// Move up one row if possible.
    pub fn event_up_one_row(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                tab.path_content.select_prev();
                if tab.line_index > 0 {
                    tab.line_index -= 1;
                }
            }
            Mode::Preview => tab.line_index = tab.window.top,
            _ => (),
        }
        tab.window.scroll_up_one(tab.line_index);
    }

    /// Move down one row if possible.
    pub fn event_down_one_row(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                tab.path_content.select_next();
                let max_line = tab.path_content.files.len();
                if max_line >= ContentWindow::WINDOW_MARGIN_TOP
                    && tab.line_index < max_line - ContentWindow::WINDOW_MARGIN_TOP
                {
                    tab.line_index += 1;
                }
            }
            Mode::Preview => tab.line_index = tab.window.bottom,
            _ => (),
        }
        tab.window.scroll_down_one(tab.line_index);
    }

    /// Move to the top of the current directory.
    pub fn event_go_top(tab: &mut Tab) {
        if let Mode::Normal = tab.mode {
            tab.path_content.select_index(0);
        }
        tab.line_index = 0;
        tab.window.scroll_to(0);
    }

    /// Move up 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves up one page.
    pub fn event_page_up(tab: &mut Tab) {
        let scroll_up: usize = if let Mode::Normal = tab.mode {
            10
        } else {
            tab.height
        };
        let up_index = if tab.line_index > scroll_up {
            tab.line_index - scroll_up
        } else {
            0
        };
        if let Mode::Normal = tab.mode {
            tab.path_content.select_index(up_index);
        }
        tab.line_index = up_index;
        tab.window.scroll_to(up_index);
    }

    /// Move to the bottom of current view.
    pub fn event_go_bottom(tab: &mut Tab) {
        let last_index: usize;
        if let Mode::Normal = tab.mode {
            last_index = tab.path_content.files.len() - 1;
            tab.path_content.select_index(last_index);
        } else {
            last_index = tab.preview.len() - 1;
        }
        tab.line_index = last_index;
        tab.window.scroll_to(last_index);
    }

    /// Move the cursor to the start of line.
    pub fn event_cursor_home(tab: &mut Tab) {
        tab.input.cursor_start()
    }

    /// Move the cursor to the end of line.
    pub fn event_cursor_end(tab: &mut Tab) {
        tab.input.cursor_end()
    }

    /// Move down 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves down one page.
    pub fn event_page_down(tab: &mut Tab) {
        let down_index: usize;
        if let Mode::Normal = tab.mode {
            down_index = min(tab.path_content.files.len() - 1, tab.line_index + 10);
            tab.path_content.select_index(down_index);
        } else {
            down_index = min(tab.preview.len() - 1, tab.line_index + 30)
        }
        tab.line_index = down_index;
        tab.window.scroll_to(down_index);
    }

    /// Select a given row, if there's something in it.
    pub fn event_select_row(status: &mut Status, row: u16) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            let tab = status.selected();
            tab.line_index = Self::row_to_index(row);
            tab.path_content.select_index(tab.line_index);
            tab.window.scroll_to(tab.line_index);
        }
        Ok(())
    }

    /// Select the next shortcut.
    pub fn event_shortcut_next(tab: &mut Tab) {
        tab.shortcut.next()
    }

    /// Select the previous shortcut.
    pub fn event_shortcut_prev(tab: &mut Tab) {
        tab.shortcut.prev()
    }

    /// Select the next element in history of visited files.
    pub fn event_history_next(tab: &mut Tab) {
        tab.history.next()
    }

    /// Select the previous element in history of visited files.
    pub fn event_history_prev(tab: &mut Tab) {
        tab.history.prev()
    }

    /// Move to parent directory if there's one.
    /// Raise an FmError in root folder.
    /// Add the starting directory to history.
    pub fn event_move_to_parent(tab: &mut Tab) -> FmResult<()> {
        let parent = tab.path_content.path.parent();
        let path = path::PathBuf::from(parent.ok_or_else(|| {
            FmError::new(
                ErrorVariant::CUSTOM("event move to parent".to_owned()),
                "Root directory has no parent",
            )
        })?);
        tab.history.push(&path);
        tab.path_content = PathContent::new(path, tab.show_hidden)?;
        tab.window.reset(tab.path_content.files.len());
        tab.line_index = 0;
        tab.input.cursor_start();
        Ok(())
    }

    /// Move the cursor left one block.
    pub fn event_move_cursor_left(tab: &mut Tab) {
        tab.input.cursor_left()
    }

    /// Open the file with configured opener or enter the directory.
    pub fn exec_file(status: &mut Status) -> FmResult<()> {
        let tab = status.selected();
        if tab.path_content.is_empty() {
            return Ok(());
        }
        if tab.path_content.is_selected_dir()? {
            tab.go_to_child()
        } else {
            Self::event_open_file(status)
        }
    }

    /// Move the cursor to the right in input string.
    pub fn event_move_cursor_right(tab: &mut Tab) {
        tab.input.cursor_right()
    }

    /// Delete the char to the left in input string.
    pub fn event_delete_char_left(tab: &mut Tab) {
        tab.input.delete_char_left()
    }

    /// Delete all chars right of the cursor in input string.
    pub fn event_delete_chars_right(tab: &mut Tab) {
        tab.input.delete_chars_right()
    }

    /// Add a char to input string, look for a possible completion.
    pub fn event_text_insert_and_complete(tab: &mut Tab, c: char) -> FmResult<()> {
        Self::event_text_insertion(tab, c);
        tab.fill_completion()
    }

    /// Enter a copy paste mode.
    pub fn event_copy_paste(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::NeedConfirmation(ConfirmedAction::Copy);
        Ok(())
    }

    /// Enter the 'move' mode.
    pub fn event_cut_paste(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::NeedConfirmation(ConfirmedAction::Move);
        Ok(())
    }

    /// Enter the new dir mode.
    pub fn event_new_dir(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::ReadInput(InputKind::Newdir);
        Ok(())
    }

    /// Enter the new file mode.
    pub fn event_new_file(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::ReadInput(InputKind::Newfile);
        Ok(())
    }

    /// Enter the execute mode. Most commands must be executed to allow for
    /// a confirmation.
    pub fn event_exec(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::Completed(CompletionKind::Exec);
        Ok(())
    }

    /// Enter preview mode.
    /// Every file can be previewed. See the `crate::enum::Preview` for
    /// more details on previewing.
    pub fn event_preview(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.files.is_empty() {
            return Err(FmError::new(
                ErrorVariant::CUSTOM("event_preview".to_owned()),
                "No file to preview",
            ));
        }
        if let Some(file) = tab.path_content.selected_file() {
            if let FileKind::NormalFile = file.file_kind {
                tab.mode = Mode::Preview;
                tab.preview = Preview::new(&tab.path_content)?;
                tab.window.reset(tab.preview.len());
            }
        }
        Ok(())
    }

    /// Enter the delete mode.
    /// A confirmation is then asked.
    pub fn event_delete_file(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::NeedConfirmation(ConfirmedAction::Delete);
        Ok(())
    }

    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn event_help(status: &mut Status) -> FmResult<()> {
        let help = status.help.clone();
        let tab = status.selected();
        tab.mode = Mode::Preview;
        tab.preview = Preview::help(help);
        tab.window.reset(tab.preview.len());
        Ok(())
    }

    /// Enter the search mode.
    /// Matching items are displayed as you type them.
    pub fn event_search(tab: &mut Tab) -> FmResult<()> {
        tab.searched = None;
        tab.mode = Mode::Completed(CompletionKind::Search);
        Ok(())
    }

    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    pub fn event_regex_match(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::ReadInput(InputKind::RegexMatch);
        Ok(())
    }

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn event_sort(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::ReadInput(InputKind::Sort);
        Ok(())
    }

    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's usefull to reset the cursor before leaving the application.
    pub fn event_quit(tab: &mut Tab) -> FmResult<()> {
        tab.must_quit = true;
        Ok(())
    }

    /// Reset the mode to normal.
    pub fn event_leave_need_confirmation(tab: &mut Tab) {
        tab.mode = Mode::Normal;
    }

    /// Sort the file with given criteria
    /// Valid kind of sorts are :
    /// by kind : directory first, files next, in alphanumeric order
    /// by filename,
    /// by date of modification,
    /// by size,
    /// by extension.
    /// The first letter is used to identify the method.
    /// If the user types an uppercase char, the sort is reverse.
    pub fn event_leave_sort(tab: &mut Tab, c: char) {
        tab.mode = Mode::Normal;
        match c {
            'k' | 'K' => tab.path_content.sort_by = SortBy::Kind,
            'n' | 'N' => tab.path_content.sort_by = SortBy::Filename,
            'm' | 'M' => tab.path_content.sort_by = SortBy::Date,
            's' | 'S' => tab.path_content.sort_by = SortBy::Size,
            'e' | 'E' => tab.path_content.sort_by = SortBy::Extension,
            'r' => tab.path_content.reverse = !tab.path_content.reverse,
            _ => {
                return;
            }
        }
        if c.is_uppercase() {
            tab.path_content.reverse = true
        }
        if !tab.path_content.files.is_empty() {
            tab.path_content.files[tab.line_index].unselect();
            tab.path_content.sort();
            if tab.path_content.reverse {
                tab.path_content.files.reverse();
            }
            Self::event_go_top(tab);
            tab.path_content.select_index(0)
        }
    }

    /// Insert a char in the input string.
    pub fn event_text_insertion(tab: &mut Tab, c: char) {
        tab.input.insert(c);
    }

    /// Toggle the display of hidden files.
    pub fn event_toggle_hidden(tab: &mut Tab) -> FmResult<()> {
        tab.show_hidden = !tab.show_hidden;
        tab.path_content.show_hidden = !tab.path_content.show_hidden;
        tab.path_content.reset_files()?;
        tab.line_index = 0;
        tab.window.reset(tab.path_content.files.len());
        Ok(())
    }

    /// Open a file with custom opener.
    pub fn event_open_file(status: &mut Status) -> FmResult<()> {
        match status.opener.open(
            status
                .selected_non_mut()
                .path_content
                .selected_file()
                .ok_or_else(|| {
                    FmError::new(
                        ErrorVariant::CUSTOM("event open file".to_owned()),
                        "Empty directory",
                    )
                })?
                .path
                .clone(),
        ) {
            Ok(_) => (),
            Err(e) => info!(
                "Error opening {:?}: {:?}",
                status.selected_non_mut().path_content.selected_file(),
                e
            ),
        }
        Ok(())
    }

    /// Enter the rename mode.
    pub fn event_rename(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::ReadInput(InputKind::Rename);
        Ok(())
    }

    /// Enter the goto mode where an user can type a path to jump to.
    pub fn event_goto(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::Completed(CompletionKind::Goto);
        tab.completion.reset();
        Ok(())
    }

    /// Open a new terminal in current directory.
    /// The shell is a fork of current process and will exit if the application
    /// is terminated first.
    pub fn event_shell(status: &mut Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        execute_in_child(
            &status.opener.terminal.clone(),
            &vec![
                "-d",
                tab.path_content.path.to_str().ok_or_else(|| {
                    FmError::new(
                        ErrorVariant::CUSTOM("event_shell".to_owned()),
                        "Couldn't parse the path name",
                    )
                })?,
            ],
        )?;
        Ok(())
    }

    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    pub fn event_history(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::History;
        Ok(())
    }

    /// Enter the shortcut mode, allowing to visite predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    pub fn event_shortcut(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::Shortcut;
        Ok(())
    }

    /// A right click opens a file or a directory.
    pub fn event_right_click(status: &mut Status, row: u16) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            let tab = status.selected();
            if tab.path_content.files.is_empty() || row as usize > tab.path_content.files.len() + 1
            {
                return Err(FmError::new(
                    ErrorVariant::CUSTOM("event right click".to_owned()),
                    "not found",
                ));
            }
            tab.line_index = Self::row_to_index(row);
            tab.path_content.select_index(tab.line_index);
            tab.window.scroll_to(tab.line_index);
            if let FileKind::Directory = tab
                .path_content
                .selected_file()
                .ok_or_else(|| {
                    FmError::new(
                        ErrorVariant::CUSTOM("event right click".to_owned()),
                        "not found",
                    )
                })?
                .file_kind
            {
                Self::exec_file(status)
            } else {
                Self::event_open_file(status)
            }
        } else {
            Ok(())
        }
    }

    /// Replace the input string by the selected completion.
    pub fn event_replace_input_with_completion(tab: &mut Tab) {
        tab.input.replace(tab.completion.current_proposition())
    }

    /// Send a signal to parent NVIM process, picking the selected file.
    /// If no RPC server were provided at launch time - which may happen for
    /// reasons unknow to me - it does nothing.
    /// It requires the "nvim-send" application to be in $PATH.
    pub fn event_nvim_filepicker(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.files.is_empty() {
            info!("Called nvim filepicker in an empty directory.");
            return Ok(());
        }
        // "nvim-send --remote-send '<esc>:e readme.md<cr>' --servername 127.0.0.1:8888"
        if let Ok(nvim_listen_address) = Self::nvim_listen_address(tab) {
            if let Some(path_str) = tab.path_content.selected_path_str() {
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
        Ok(())
    }

    /// Copy the selected filename to the clipboard. Only the filename.
    pub fn event_filename_to_clipboard(tab: &Tab) -> FmResult<()> {
        if let Some(file) = tab.path_content.selected_file() {
            let filename = file.filename.clone();
            let mut ctx = ClipboardContext::new()?;
            ctx.set_contents(filename)?;
            // For some reason, it's not writen if you don't read it back...
            let _ = ctx.get_contents();
        }
        Ok(())
    }

    /// Copy the selected filepath to the clipboard. The absolute path.
    pub fn event_filepath_to_clipboard(tab: &Tab) -> FmResult<()> {
        if let Some(filepath) = tab.path_content.selected_path_str() {
            let mut ctx = ClipboardContext::new()?;
            ctx.set_contents(filepath)?;
            // For some reason, it's not writen if you don't read it back...
            let _ = ctx.get_contents();
        }
        Ok(())
    }

    /// Enter the filter mode, where you can filter.
    /// See `crate::filter::Filter` for more details.
    pub fn event_filter(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::ReadInput(InputKind::Filter);
        Ok(())
    }

    /// Move back in history to the last visited directory.
    pub fn event_back(tab: &mut Tab) -> FmResult<()> {
        if tab.history.visited.len() <= 1 {
            return Ok(());
        }
        tab.history.visited.pop();
        let last = tab.history.visited[tab.history.len() - 1].clone();
        tab.set_pathcontent(last)?;

        Ok(())
    }

    /// Move to $HOME aka ~.
    pub fn event_home(tab: &mut Tab) -> FmResult<()> {
        let home_cow = shellexpand::tilde("~");
        let home: &str = home_cow.borrow();
        let path = std::fs::canonicalize(home)?;
        tab.set_pathcontent(path)?;

        Ok(())
    }

    fn nvim_listen_address(tab: &Tab) -> Result<String, std::env::VarError> {
        if !tab.nvim_server.is_empty() {
            Ok(tab.nvim_server.clone())
        } else {
            std::env::var("NVIM_LISTEN_ADDRESS")
        }
    }

    /// Execute a rename of the selected file.
    /// It uses the `fs::rename` function and has the same limitations.
    /// We only tries to rename in the same directory, so it shouldn't be a problem.
    /// Filename is sanitized before processing.
    pub fn exec_rename(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.files.is_empty() {
            return Err(FmError::new(
                ErrorVariant::CUSTOM("event rename".to_owned()),
                "Empty directory",
            ));
        }
        fs::rename(
            tab.path_content.selected_path_str().ok_or_else(|| {
                FmError::new(
                    ErrorVariant::CUSTOM("exec rename".to_owned()),
                    "File not found",
                )
            })?,
            tab.path_content
                .path
                .to_path_buf()
                .join(&sanitize_filename::sanitize(tab.input.string())),
        )?;
        tab.refresh_view()
    }

    /// Creates a new file with input string as name.
    /// We use `fs::File::create` internally, so if the file already exists,
    /// it will be overwritten.
    /// Filename is sanitized before processing.
    pub fn exec_newfile(tab: &mut Tab) -> FmResult<()> {
        fs::File::create(
            tab.path_content
                .path
                .join(sanitize_filename::sanitize(tab.input.string())),
        )?;
        tab.refresh_view()
    }

    /// Creates a new directory with input string as name.
    /// We use `fs::create_dir` internally so it will fail if the input string
    /// is not an end point in the file system.
    /// ie. the user can create `newdir` but not `newdir/newfolder`.
    /// It will also fail if the directory already exists.
    /// Directory name is sanitized before processing.
    pub fn exec_newdir(tab: &mut Tab) -> FmResult<()> {
        match fs::create_dir(
            tab.path_content
                .path
                .join(sanitize_filename::sanitize(tab.input.string())),
        ) {
            Ok(()) => (),
            Err(e) => match e.kind() {
                std::io::ErrorKind::AlreadyExists => (),
                _ => return Err(FmError::from(e)),
            },
        }
        tab.refresh_view()
    }

    /// Tries to execute the selected file with an executable which is read
    /// from the input string. It will fail silently if the executable can't
    /// be found.
    /// Optional parameters can be passed normally. ie. `"ls -lah"`
    pub fn exec_exec(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.files.is_empty() {
            return Err(FmError::new(
                ErrorVariant::CUSTOM("exec exec".to_owned()),
                "empty directory",
            ));
        }
        let exec_command = tab.input.string();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        if std::path::Path::new(command).exists() {
            let path = &tab.path_content.selected_path_str().ok_or_else(|| {
                FmError::new(
                    ErrorVariant::CUSTOM("exec exec".to_owned()),
                    &format!("can't find command {}", command),
                )
            })?;
            args.push(path);
            execute_in_child(command, &args)?;
            tab.completion.reset();
            tab.input.reset();
        }
        Ok(())
    }

    /// Executes a `dragon-drop` command on the selected file.
    /// It obviously requires the `dragon-drop` command to be installed.
    pub fn event_drag_n_drop(status: &mut Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        execute_in_child(
            DEFAULT_DRAGNDROP,
            &vec![&tab.path_content.selected_path_str().ok_or_else(|| {
                FmError::new(
                    ErrorVariant::CUSTOM("event drag n drop".to_owned()),
                    "can't find dragon-drop in the system. Is the application installed?",
                )
            })?],
        )?;
        Ok(())
    }

    /// Executes a search in current folder, selecting the first file matching
    /// the current completion proposition.
    /// ie. If you typed `"jpg"` before, it will move to the first file
    /// whose filename contains `"jpg"`.
    /// The current order of files is used.
    pub fn exec_search(tab: &mut Tab) {
        let searched = tab.input.string();
        tab.input.reset();
        if searched.is_empty() {
            tab.searched = None;
            return;
        }
        tab.searched = Some(searched.clone());
        let next_index = tab.line_index;
        Self::search_from(tab, searched, next_index);
    }

    /// Search in current directory for an file whose name contains `searched_name`,
    /// from a starting position `next_index`.
    /// We search forward from that position and start again from top if nothing is found.
    /// We move the selection to the first matching file.
    fn search_from(tab: &mut Tab, searched_name: String, mut next_index: usize) {
        let mut found = false;
        for (index, file) in tab.path_content.files.iter().enumerate().skip(next_index) {
            if file.filename.contains(&searched_name) {
                next_index = index;
                found = true;
                break;
            };
        }
        if found {
            tab.path_content.select_index(next_index);
            tab.line_index = next_index;
            tab.window.scroll_to(tab.line_index);
        } else {
            for (index, file) in tab.path_content.files.iter().enumerate().take(next_index) {
                if file.filename.starts_with(&searched_name) {
                    next_index = index;
                    found = true;
                    break;
                };
            }
            if found {
                tab.path_content.select_index(next_index);
                tab.line_index = next_index;
                tab.window.scroll_to(tab.line_index);
            }
        }
    }

    pub fn event_search_next(tab: &mut Tab) -> FmResult<()> {
        if let Some(searched) = tab.searched.clone() {
            let next_index = (tab.line_index + 1) % tab.path_content.files.len();
            Self::search_from(tab, searched, next_index);
        } else {
        }
        Ok(())
    }

    /// Move to the folder typed by the user.
    /// The first completion proposition is used, `~` expansion is done.
    /// If no result were found, no cd is done and we go back to normal mode
    /// silently.
    pub fn exec_goto(tab: &mut Tab) -> FmResult<()> {
        if tab.completion.is_empty() {
            return Ok(());
        }
        let completed = tab.completion.current_proposition();
        let path = string_to_path(completed)?;
        tab.input.reset();
        tab.history.push(&path);
        tab.path_content = PathContent::new(path, tab.show_hidden)?;
        tab.window.reset(tab.path_content.files.len());
        Ok(())
    }

    /// Move to the selected shortcut.
    /// It may fail if the user has no permission to visit the path.
    pub fn exec_shortcut(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        let path = tab.shortcut.selected();
        tab.history.push(&path);
        tab.path_content = PathContent::new(path, tab.show_hidden)?;
        Self::event_normal(tab)
    }

    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    pub fn exec_history(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        tab.path_content = PathContent::new(
            tab.history.selected().ok_or_else(|| {
                FmError::new(
                    ErrorVariant::CUSTOM("exec history".to_owned()),
                    "path unreachable",
                )
            })?,
            tab.show_hidden,
        )?;
        tab.history.drop_queue();
        Self::event_normal(tab)
    }

    /// Apply a filter to the displayed files.
    /// See `crate::filter` for more details.
    pub fn exec_filter(tab: &mut Tab) -> FmResult<()> {
        let filter = FilterKind::from_input(&tab.input.string());
        tab.path_content.set_filter(filter);
        tab.input.reset();
        tab.path_content.reset_files()?;
        Self::event_normal(tab)
    }

    /// Move up one row in modes allowing movement.
    /// Does nothing if the selected item is already the first in list.
    pub fn event_move_up(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_up_one_row(status.selected()),
            Mode::Jump => EventExec::event_jumplist_prev(status),
            Mode::History => EventExec::event_history_prev(status.selected()),
            Mode::Shortcut => EventExec::event_shortcut_prev(status.selected()),
            Mode::Completed(_) => {
                status.selected().completion.prev();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move down one row in modes allowing movements.
    /// Does nothing if the user is already at the bottom.
    pub fn event_move_down(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_down_one_row(status.selected()),
            Mode::Jump => EventExec::event_jumplist_next(status),
            Mode::History => EventExec::event_history_next(status.selected()),
            Mode::Shortcut => EventExec::event_shortcut_next(status.selected()),
            Mode::Completed(_) => status.selected().completion.next(),
            _ => (),
        };
        Ok(())
    }

    /// Move to parent in normal mode,
    /// move left one char in mode requiring text input.
    pub fn event_move_left(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => EventExec::event_move_to_parent(status.selected()),
            Mode::ReadInput(_) | Mode::Completed(_) => {
                EventExec::event_move_cursor_left(status.selected());
                Ok(())
            }

            _ => Ok(()),
        }
    }

    /// Move to child if any or open a regular file in normal mode.
    /// Move the cursor one char to right in mode requiring text input.
    pub fn event_move_right(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => EventExec::exec_file(status),
            Mode::ReadInput(_) | Mode::Completed(_) => {
                EventExec::event_move_cursor_right(status.selected());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Delete a char to the left in modes allowing edition.
    pub fn event_backspace(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::ReadInput(_) | Mode::Completed(_) => {
                EventExec::event_delete_char_left(status.selected());
                Ok(())
            }
            Mode::Normal => Ok(()),
            _ => Ok(()),
        }
    }

    /// Delete all chars to the right in mode allowing edition.
    pub fn event_delete(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::ReadInput(_) | Mode::Completed(_) => {
                EventExec::event_delete_chars_right(status.selected());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Move to leftmost char in mode allowing edition.
    pub fn event_key_home(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_go_top(status.selected()),
            _ => EventExec::event_cursor_home(status.selected()),
        };
        Ok(())
    }

    /// Move to the bottom in any mode.
    pub fn event_end(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_go_bottom(status.selected()),
            _ => EventExec::event_cursor_end(status.selected()),
        };
        Ok(())
    }

    /// Move up 10 lines in normal mode and preview.
    pub fn page_up(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_page_up(status.selected()),
            _ => (),
        };
        Ok(())
    }

    /// Move down 10 lines in normal & preview mode.
    pub fn page_down(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_page_down(status.selected()),
            _ => (),
        };
        Ok(())
    }

    /// Execute the mode.
    /// In modes requiring confirmation or text input, it will execute the
    /// related action.
    /// In normal mode, it will open the file.
    /// Reset to normal mode afterwards.
    pub fn enter(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::ReadInput(InputKind::Rename) => EventExec::exec_rename(status.selected())?,
            Mode::ReadInput(InputKind::Newfile) => EventExec::exec_newfile(status.selected())?,
            Mode::ReadInput(InputKind::Newdir) => EventExec::exec_newdir(status.selected())?,
            Mode::ReadInput(InputKind::Chmod) => EventExec::exec_chmod(status)?,
            Mode::ReadInput(InputKind::RegexMatch) => EventExec::exec_regex(status)?,
            Mode::ReadInput(InputKind::Filter) => EventExec::exec_filter(status.selected())?,
            Mode::Jump => EventExec::exec_jump(status)?,
            Mode::Completed(CompletionKind::Exec) => EventExec::exec_exec(status.selected())?,
            Mode::Completed(CompletionKind::Search) => EventExec::exec_search(status.selected()),
            Mode::Completed(CompletionKind::Goto) => EventExec::exec_goto(status.selected())?,
            Mode::History => EventExec::exec_history(status.selected())?,
            Mode::Shortcut => EventExec::exec_shortcut(status.selected())?,
            Mode::Normal => EventExec::exec_file(status)?,
            Mode::NeedConfirmation(_)
            | Mode::Preview
            | Mode::Completed(CompletionKind::Nothing)
            | Mode::ReadInput(InputKind::Sort)
            | Mode::ReadInput(InputKind::Marks(_)) => (),
        };

        status.selected().input.reset();
        status.selected().mode = Mode::Normal;
        Ok(())
    }

    /// Change tab in normal mode with dual pane displayed,
    /// insert a completion in modes allowing completion.
    pub fn tab(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Completed(_) => EventExec::event_replace_input_with_completion(status.selected()),
            Mode::Normal => status.next(),
            _ => (),
        };
        Ok(())
    }

    /// Change tab in normal mode.
    pub fn backtab(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected().mode {
            status.prev()
        }
        Ok(())
    }

    /// Start a fuzzy find with skim.
    /// ATM idk how to avoid using the whole screen.
    pub fn event_fuzzyfind(status: &mut Status) -> FmResult<()> {
        status.fill_tabs_with_skim()
    }

    /// Copy the filename of the selected file in normal mode.
    pub fn event_copy_filename(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return EventExec::event_filename_to_clipboard(status.selected());
        }
        Ok(())
    }

    /// Copy the filepath of the selected file in normal mode.
    pub fn event_copy_filepath(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return EventExec::event_filepath_to_clipboard(status.selected());
        }
        Ok(())
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn event_refreshview(status: &mut Status) -> FmResult<()> {
        Self::refresh_selected_view(status)
    }

    /// Open a thumbnail of an image, scaled up to the whole window.
    pub fn event_thumbnail(tab: &mut Tab) -> FmResult<()> {
        if let Mode::Normal = tab.mode {
            tab.mode = Mode::Preview;
            if let Some(file_info) = tab.path_content.selected_file() {
                tab.preview = Preview::thumbnail(file_info.path.to_owned())?;
                tab.window.reset(tab.preview.len());
            }
        }
        Ok(())
    }

    /// Toggle between a full display (aka ls -lah) or a simple mode (only the
    /// filenames).
    pub fn event_toggle_display_full(status: &mut Status) -> FmResult<()> {
        status.display_full = !status.display_full;
        Ok(())
    }

    /// Toggle between dualpane and single pane. Does nothing if the width
    /// is too low to display both panes.
    pub fn event_toggle_dualpane(status: &mut Status) -> FmResult<()> {
        status.dual_pane = !status.dual_pane;
        status.select_tab(0)?;
        Ok(())
    }

    fn row_to_index(row: u16) -> usize {
        row as usize - RESERVED_ROWS
    }
}

fn string_to_path(path_string: String) -> FmResult<path::PathBuf> {
    let expanded_cow_path = shellexpand::tilde(&path_string);
    let expanded_target: &str = expanded_cow_path.borrow();
    Ok(std::fs::canonicalize(expanded_target)?)
}
