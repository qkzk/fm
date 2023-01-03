use std::borrow::Borrow;
use std::cmp::min;
use std::fs;
use std::path;

use copypasta::{ClipboardContext, ClipboardProvider};
use log::info;
use sysinfo::SystemExt;

use crate::bulkrename::Bulkrename;
use crate::completion::InputCompleted;
use crate::constant_strings_paths::DEFAULT_DRAGNDROP;
use crate::constant_strings_paths::NVIM_RPC_SENDER;
use crate::content_window::RESERVED_ROWS;
use crate::copy_move::CopyMove;
use crate::fileinfo::FileKind;
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::mode::Navigate;
use crate::mode::{InputSimple, MarkAction, Mode, NeedConfirmation};
use crate::opener::execute_in_child;
use crate::preview::Preview;
use crate::selectable_content::SelectableContent;
use crate::status::Status;
use crate::tab::Tab;
use crate::term_manager::MIN_WIDTH_FOR_DUAL_PANE;
use crate::utils::disk_used_by_path;

/// Every kind of mutation of the application is defined here.
/// It mutates `Status` or its children `Tab`.
pub struct EventExec {}

impl EventExec {
    /// Reset the selected tab view to the default.
    pub fn refresh_status(status: &mut Status) -> FmResult<()> {
        status.refresh_users()?;
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
        Self::refresh_status(status)?;
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
            .content
            .iter()
            .for_each(|file| {
                status.flagged.push(file.path.clone());
            });
        status.reset_tabs_view()
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn event_reverse_flags(status: &mut Status) -> FmResult<()> {
        status.tabs[status.index]
            .path_content
            .content
            .iter()
            .for_each(|file| status.flagged.toggle(&file.path));
        status.reset_tabs_view()
    }

    /// Toggle a single flag and move down one row.
    pub fn event_toggle_flag(status: &mut Status) -> FmResult<()> {
        if let Some(file) = status.selected().path_content.selected() {
            let path = file.path.clone();
            status.toggle_flag_on_path(&path);
            Self::event_down_one_row(status.selected());
        }
        Ok(())
    }

    /// Move to the next file in the jump list.
    pub fn event_jumplist_next(status: &mut Status) {
        status.flagged.next()
    }

    /// Move to the previous file in the jump list.
    pub fn event_jumplist_prev(status: &mut Status) {
        status.flagged.prev()
    }

    /// Change to CHMOD mode allowing to edit permissions of a file.
    pub fn event_chmod(status: &mut Status) -> FmResult<()> {
        if status.selected().path_content.is_empty() {
            return Ok(());
        }
        status.selected().mode = Mode::InputSimple(InputSimple::Chmod);
        if status.flagged.is_empty() {
            status.flagged.push(
                status.tabs[status.index]
                    .path_content
                    .selected()
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
            status.flagged.index = 0;
            status.selected().mode = Mode::Navigate(Navigate::Jump)
        }
        Ok(())
    }

    /// Enter Marks new mode, allowing to bind a char to a path.
    pub fn event_marks_new(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputSimple(InputSimple::Marks(MarkAction::New));
        Ok(())
    }

    /// Enter Marks jump mode, allowing to jump to a marked file.
    pub fn event_marks_jump(status: &mut Status) -> FmResult<()> {
        if status.marks.is_empty() {
            return Ok(());
        }
        status.selected().mode = Mode::InputSimple(InputSimple::Marks(MarkAction::Jump));
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
            let path = path.clone();
            status.selected().set_pathcontent(&path)?
        }
        Self::event_normal(status.selected())
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn event_symlink(status: &mut Status) -> FmResult<()> {
        for oldpath in status.flagged.content.iter() {
            let filename = oldpath
                .as_path()
                .file_name()
                .ok_or_else(|| FmError::custom("event symlink", "File not found"))?;
            let newpath = status.tabs[status.index]
                .path_content
                .path
                .clone()
                .join(filename);
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
        for pathbuf in status.flagged.content.iter() {
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
            for path in status.flagged.content.iter() {
                Status::set_permissions(path, permissions)?
            }
            status.flagged.clear()
        }
        status.selected().refresh_view()?;
        status.reset_tabs_view()
    }

    /// Execute a jump to the selected flagged file.
    /// If the user selected a directory, we jump inside it.
    /// Otherwise, we jump to the parent and select the file.
    pub fn exec_jump(status: &mut Status) -> FmResult<()> {
        if let Some(jump_target) = status.flagged.selected() {
            let jump_target = jump_target.to_owned();
            let target_dir = match jump_target.parent() {
                Some(parent) => parent,
                None => &jump_target,
            };
            let tab = status.selected();
            tab.input.clear();
            tab.history.push(target_dir);
            tab.path_content.change_directory(target_dir)?;
            let index = tab.find_jump_index(&jump_target).unwrap_or_default();
            tab.path_content.select_index(index);
            tab.set_window();
            tab.scroll_to(index);
        }
        Ok(())
    }

    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    pub fn exec_confirmed_action(
        status: &mut Status,
        confirmed_action: NeedConfirmation,
    ) -> FmResult<()> {
        Self::_exec_confirmed_action(status, confirmed_action)?;
        status.selected().mode = Mode::Normal;
        Ok(())
    }

    fn _exec_confirmed_action(
        status: &mut Status,
        confirmed_action: NeedConfirmation,
    ) -> FmResult<()> {
        match confirmed_action {
            NeedConfirmation::Delete => Self::exec_delete_files(status),
            NeedConfirmation::Move => Self::exec_cut_paste(status),
            NeedConfirmation::Copy => Self::exec_copy_paste(status),
            NeedConfirmation::EmptyTrash => Self::exec_trash_empty(status),
        }
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
        tab.window.reset(tab.path_content.content.len());
        tab.mode = Mode::Normal;
        tab.preview = Preview::new_empty();
        Ok(())
    }

    /// Move up one row if possible.
    pub fn event_up_one_row(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                tab.path_content.unselect_current();
                tab.path_content.prev();
                tab.path_content.select_current();
                tab.window.scroll_up_one(tab.path_content.index)
            }
            Mode::Preview => tab.window.scroll_up_one(tab.window.top),
            _ => (),
        }
    }

    /// Move down one row if possible.
    pub fn event_down_one_row(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                tab.path_content.unselect_current();
                tab.path_content.next();
                tab.path_content.select_current();
                tab.window.scroll_down_one(tab.path_content.index)
            }
            Mode::Preview => tab.window.scroll_down_one(tab.window.bottom),
            _ => (),
        }
    }

    /// Move to the top of the current directory.
    pub fn event_go_top(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => tab.path_content.select_index(0),
            Mode::Preview => (),
            _ => {
                return;
            }
        }
        tab.window.scroll_to(0);
    }

    /// Move up 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves up one page.
    pub fn event_page_up(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                let up_index = if tab.path_content.index > 10 {
                    tab.path_content.index - 10
                } else {
                    0
                };
                tab.path_content.select_index(up_index);
                tab.window.scroll_to(up_index)
            }
            Mode::Preview => {
                if tab.window.top > 0 {
                    let skip = min(tab.window.top, 30);
                    tab.window.bottom -= skip;
                    tab.window.top -= skip;
                }
            }
            _ => (),
        }
    }

    /// Move down 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves down one page.
    pub fn event_page_down(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                let down_index = min(
                    tab.path_content.content.len() - 1,
                    tab.path_content.index + 10,
                );
                tab.path_content.select_index(down_index);
                tab.window.scroll_to(down_index);
            }
            Mode::Preview => {
                if tab.window.bottom < tab.preview.len() {
                    let skip = min(tab.preview.len() - tab.window.bottom, 30);
                    tab.window.bottom += skip;
                    tab.window.top += skip;
                }
            }
            _ => (),
        }
    }
    /// Move to the bottom of current view.
    pub fn event_go_bottom(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                let last_index = tab.path_content.content.len() - 1;
                tab.path_content.select_index(last_index);
                tab.window.scroll_to(last_index)
            }
            Mode::Preview => tab.window.scroll_to(tab.preview.len() - 1),
            _ => (),
        }
    }

    /// Move the cursor to the start of line.
    pub fn event_cursor_home(tab: &mut Tab) {
        tab.input.cursor_start()
    }

    /// Move the cursor to the end of line.
    pub fn event_cursor_end(tab: &mut Tab) {
        tab.input.cursor_end()
    }

    /// Select a given row, if there's something in it.
    pub fn event_select_row(status: &mut Status, row: u16) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            let tab = status.selected();
            let index = Self::row_to_index(row);
            tab.path_content.select_index(index);
            tab.window.scroll_to(index);
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
    /// Watchout! Since the history is displayed in reverse order,
    /// we call the "prev" method of the `History` instance instead.
    pub fn event_history_next(tab: &mut Tab) {
        tab.history.next()
    }

    /// Select the previous element in history of visited files.
    /// Watchout! Since the history is displayed in reverse order,
    /// we call the "next" method of the `History` instance instead.
    pub fn event_history_prev(tab: &mut Tab) {
        tab.history.prev()
    }

    /// Move to parent directory if there's one.
    /// Does
    /// Add the starting directory to history.
    pub fn event_move_to_parent(tab: &mut Tab) -> FmResult<()> {
        let path = tab.path_content.path.clone();
        if let Some(parent) = path.parent() {
            tab.set_pathcontent(parent)?;
        }
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
    /// A confirmation is asked before copying all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn event_copy_paste(status: &mut Status) -> FmResult<()> {
        if status.flagged.is_empty() {
            return Ok(());
        }
        status.selected().mode = Mode::NeedConfirmation(NeedConfirmation::Copy);
        Ok(())
    }

    /// Enter the 'move' mode.
    /// A confirmation is asked before moving all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn event_cut_paste(status: &mut Status) -> FmResult<()> {
        if status.flagged.is_empty() {
            return Ok(());
        }
        status.selected().mode = Mode::NeedConfirmation(NeedConfirmation::Move);
        Ok(())
    }

    /// Enter the new dir mode.
    pub fn event_new_dir(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputSimple(InputSimple::Newdir);
        Ok(())
    }

    /// Enter the new file mode.
    pub fn event_new_file(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputSimple(InputSimple::Newfile);
        Ok(())
    }

    /// Enter the execute mode. Most commands must be executed to allow for
    /// a confirmation.
    pub fn event_exec(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputCompleted(InputCompleted::Exec);
        Ok(())
    }

    /// Preview the selected file.
    /// Every file can be previewed. See the `crate::enum::Preview` for
    /// more details on previewinga file.
    /// Does nothing if the directory is empty.
    pub fn event_preview(status: &mut Status) -> FmResult<()> {
        if status.selected_non_mut().path_content.content.is_empty() {
            return Ok(());
        }
        let unmutable_tab = status.selected_non_mut();
        if let Some(file_info) = unmutable_tab.path_content.selected() {
            match file_info.file_kind {
                FileKind::Directory | FileKind::NormalFile => {
                    let preview =
                        Preview::new(file_info, &unmutable_tab.path_content.users_cache, status)?;
                    status.selected().mode = Mode::Preview;
                    status.selected().window.reset(preview.len());
                    status.selected().preview = preview;
                }
                _ => (),
            }
        }
        Ok(())
    }

    /// Enter the delete mode.
    /// A confirmation is then asked before deleting all the flagged files.
    /// Does nothing is no file is flagged.
    pub fn event_delete_file(status: &mut Status) -> FmResult<()> {
        if status.flagged.is_empty() {
            return Ok(());
        }
        status.selected().mode = Mode::NeedConfirmation(NeedConfirmation::Delete);
        Ok(())
    }

    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn event_help(status: &mut Status) -> FmResult<()> {
        let help = status.help.clone();
        let tab = status.selected();
        tab.mode = Mode::Preview;
        tab.preview = Preview::help(&help);
        tab.window.reset(tab.preview.len());
        Ok(())
    }

    /// Enter the search mode.
    /// Matching items are displayed as you type them.
    pub fn event_search(tab: &mut Tab) -> FmResult<()> {
        tab.searched = None;
        tab.mode = Mode::InputCompleted(InputCompleted::Search);
        Ok(())
    }

    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    pub fn event_regex_match(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputSimple(InputSimple::RegexMatch);
        Ok(())
    }

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn event_sort(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputSimple(InputSimple::Sort);
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
        if tab.path_content.content.is_empty() {
            return;
        }
        tab.path_content.unselect_current();
        tab.path_content.update_sort_from_char(c);
        tab.path_content.sort();
        Self::event_go_top(tab);
        tab.path_content.select_index(0)
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
        tab.window.reset(tab.path_content.content.len());
        Ok(())
    }

    /// Open a file with custom opener.
    pub fn event_open_file(status: &mut Status) -> FmResult<()> {
        match status.opener.open(
            &status
                .selected_non_mut()
                .path_content
                .selected()
                .ok_or_else(|| FmError::custom("event open file", "Empty directory"))?
                .path,
        ) {
            Ok(_) => (),
            Err(e) => info!(
                "Error opening {:?}: {:?}",
                status.selected_non_mut().path_content.selected(),
                e
            ),
        }
        Ok(())
    }

    /// Enter the rename mode.
    pub fn event_rename(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputSimple(InputSimple::Rename);
        Ok(())
    }

    /// Enter the goto mode where an user can type a path to jump to.
    pub fn event_goto(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::InputCompleted(InputCompleted::Goto);
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
                    FmError::custom("event_shell", "Couldn't parse the path name")
                })?,
            ],
        )?;
        Ok(())
    }

    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    pub fn event_history(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::Navigate(Navigate::History);
        Ok(())
    }

    /// Enter the shortcut mode, allowing to visit predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    pub fn event_shortcut(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::Navigate(Navigate::Shortcut);
        Ok(())
    }

    /// A right click opens a file or a directory.
    pub fn event_right_click(status: &mut Status, row: u16) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            let tab = status.selected();
            if tab.path_content.content.is_empty()
                || row as usize > tab.path_content.content.len() + 1
            {
                return Err(FmError::custom("event right click", "not found"));
            }
            let index = Self::row_to_index(row);
            tab.path_content.select_index(index);
            tab.window.scroll_to(index);
            if let FileKind::Directory = tab
                .path_content
                .selected()
                .ok_or_else(|| FmError::custom("event right click", "not found"))?
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
        if tab.path_content.content.is_empty() {
            info!("Called nvim filepicker in an empty directory.");
            return Ok(());
        }
        // "nvim-send --remote-send '<esc>:e readme.md<cr>' --servername 127.0.0.1:8888"
        if let Ok(nvim_listen_address) = Self::nvim_listen_address(tab) {
            if let Some(path_str) = tab.path_content.selected_path_string() {
                let _ = execute_in_child(
                    NVIM_RPC_SENDER,
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
        if let Some(file) = tab.path_content.selected() {
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
        if let Some(filepath) = tab.path_content.selected_path_string() {
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
        tab.mode = Mode::InputSimple(InputSimple::Filter);
        Ok(())
    }

    /// Move back in history to the last visited directory.
    pub fn event_back(tab: &mut Tab) -> FmResult<()> {
        if tab.history.content.len() <= 1 {
            return Ok(());
        }
        tab.history.content.pop();
        let index = tab.history.len() - 1;
        let last = tab.history.content[index].clone();
        tab.set_pathcontent(&last)?;

        Ok(())
    }

    /// Move to $HOME aka ~.
    pub fn event_home(tab: &mut Tab) -> FmResult<()> {
        let home_cow = shellexpand::tilde("~");
        let home: &str = home_cow.borrow();
        let path = std::fs::canonicalize(home)?;
        tab.set_pathcontent(&path)?;

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
        if tab.path_content.content.is_empty() {
            return Err(FmError::custom("event rename", "Empty directory"));
        }
        fs::rename(
            tab.path_content
                .selected_path_string()
                .ok_or_else(|| FmError::custom("exec rename", "File not found"))?,
            tab.path_content
                .path
                .to_path_buf()
                .join(sanitize_filename::sanitize(tab.input.string())),
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
        if tab.path_content.content.is_empty() {
            return Err(FmError::custom("exec exec", "empty directory"));
        }
        let exec_command = tab.input.string();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        if std::path::Path::new(command).exists() {
            let path = &tab.path_content.selected_path_string().ok_or_else(|| {
                FmError::custom("exec exec", &format!("can't find command {}", command))
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
            &vec![&tab.path_content.selected_path_string().ok_or_else(|| {
                FmError::custom(
                    "event drag n drop",
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
        let next_index = tab.path_content.index;
        tab.search_from(&searched, next_index);
    }

    pub fn event_search_next(tab: &mut Tab) -> FmResult<()> {
        if let Some(searched) = tab.searched.clone() {
            let next_index = (tab.path_content.index + 1) % tab.path_content.content.len();
            tab.search_from(&searched, next_index);
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
        tab.set_pathcontent(&path)?;
        tab.window.reset(tab.path_content.content.len());
        Ok(())
    }

    /// Move to the selected shortcut.
    /// It may fail if the user has no permission to visit the path.
    pub fn exec_shortcut(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        let path = tab
            .shortcut
            .selected()
            .ok_or_else(|| FmError::custom("exec shortcut", "empty shortcuts"))?
            .clone();
        tab.history.push(&path);
        tab.set_pathcontent(&path)?;
        Self::event_normal(tab)
    }

    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    pub fn exec_history(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        let path = tab
            .history
            .selected()
            .ok_or_else(|| FmError::custom("exec history", "path unreachable"))?
            .clone();
        tab.set_pathcontent(&path)?;
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
            Mode::Navigate(Navigate::Jump) => EventExec::event_jumplist_prev(status),
            Mode::Navigate(Navigate::History) => EventExec::event_history_prev(status.selected()),
            Mode::Navigate(Navigate::Trash) => EventExec::event_trash_prev(status),
            Mode::Navigate(Navigate::Shortcut) => EventExec::event_shortcut_prev(status.selected()),
            Mode::Tree => EventExec::event_select_prev_sibling(status)?,
            Mode::InputCompleted(_) => {
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
            Mode::Navigate(Navigate::Jump) => EventExec::event_jumplist_next(status),
            Mode::Navigate(Navigate::History) => EventExec::event_history_next(status.selected()),
            Mode::Navigate(Navigate::Trash) => EventExec::event_trash_next(status),
            Mode::Navigate(Navigate::Shortcut) => EventExec::event_shortcut_next(status.selected()),
            Mode::InputCompleted(_) => status.selected().completion.next(),
            Mode::Tree => EventExec::event_select_next_sibling(status)?,
            _ => (),
        };
        Ok(())
    }

    /// Move to parent in normal mode,
    /// move left one char in mode requiring text input.
    pub fn event_move_left(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => EventExec::event_move_to_parent(status.selected()),
            Mode::Tree => EventExec::event_select_parent(status),
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
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
            Mode::Tree => EventExec::event_select_first_child(status),
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
                EventExec::event_move_cursor_right(status.selected());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Delete a char to the left in modes allowing edition.
    pub fn event_backspace(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
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
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
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
            Mode::Tree => EventExec::event_tree_go_to_root(status)?,
            _ => EventExec::event_cursor_home(status.selected()),
        };
        Ok(())
    }

    /// Move to the bottom in any mode.
    pub fn event_end(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_go_bottom(status.selected()),
            Mode::Tree => EventExec::event_tree_go_to_bottom_leaf(status)?,
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
            Mode::InputSimple(InputSimple::Rename) => EventExec::exec_rename(status.selected())?,
            Mode::InputSimple(InputSimple::Newfile) => EventExec::exec_newfile(status.selected())?,
            Mode::InputSimple(InputSimple::Newdir) => EventExec::exec_newdir(status.selected())?,
            Mode::InputSimple(InputSimple::Chmod) => EventExec::exec_chmod(status)?,
            Mode::InputSimple(InputSimple::RegexMatch) => EventExec::exec_regex(status)?,
            Mode::InputSimple(InputSimple::Filter) => EventExec::exec_filter(status.selected())?,
            Mode::Navigate(Navigate::Jump) => EventExec::exec_jump(status)?,
            Mode::Navigate(Navigate::History) => EventExec::exec_history(status.selected())?,
            Mode::Navigate(Navigate::Shortcut) => EventExec::exec_shortcut(status.selected())?,
            Mode::Navigate(Navigate::Trash) => EventExec::event_trash_restore_file(status)?,
            Mode::InputCompleted(InputCompleted::Exec) => EventExec::exec_exec(status.selected())?,
            Mode::InputCompleted(InputCompleted::Search) => {
                EventExec::exec_search(status.selected())
            }
            Mode::InputCompleted(InputCompleted::Goto) => EventExec::exec_goto(status.selected())?,
            Mode::Normal => EventExec::exec_file(status)?,
            Mode::Tree => EventExec::exec_tree(status.selected())?,
            Mode::NeedConfirmation(_)
            | Mode::Preview
            | Mode::InputCompleted(InputCompleted::Nothing)
            | Mode::InputSimple(InputSimple::Sort)
            | Mode::InputSimple(InputSimple::Marks(_)) => (),
        };

        status.selected().input.reset();
        status.selected().mode = Mode::Normal;
        Ok(())
    }

    /// Change tab in normal mode with dual pane displayed,
    /// insert a completion in modes allowing completion.
    pub fn tab(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::InputCompleted(_) => {
                EventExec::event_replace_input_with_completion(status.selected())
            }
            Mode::Normal | Mode::Tree => status.next(),
            _ => (),
        };
        Ok(())
    }

    /// Change tab in normal mode.
    pub fn backtab(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Tree => status.prev(),
            _ => (),
        };
        Ok(())
    }

    /// Start a fuzzy find with skim.
    /// ATM idk how to avoid using the whole screen.
    pub fn event_fuzzyfind(status: &mut Status) -> FmResult<()> {
        status.skim_output_to_tab()
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
        Self::refresh_status(status)
    }

    /// Open a thumbnail of an image, scaled up to the whole window.
    pub fn event_thumbnail(tab: &mut Tab) -> FmResult<()> {
        if let Mode::Normal = tab.mode {
            tab.mode = Mode::Preview;
            if let Some(file_info) = tab.path_content.selected() {
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

    /// Move flagged files to the trash directory.
    /// More information in the trash crate itself.
    /// If the file is mounted on the $topdir of the trash (aka the $HOME mount point),
    /// it is moved there.
    /// Else, nothing is done.
    pub fn event_trash_move_file(status: &mut Status) -> FmResult<()> {
        let trash_mount_point = disk_used_by_path(
            status.system_info.disks(),
            &std::path::PathBuf::from(&status.trash.trash_folder_files),
        );

        for flagged in status.flagged.content.iter() {
            let origin_mount_point = disk_used_by_path(status.disks(), flagged);
            if trash_mount_point != origin_mount_point {
                continue;
            }
            status.trash.trash(flagged)?;
        }
        status.flagged.clear();
        status.selected().refresh_view()?;
        Ok(())
    }

    /// Restore a file from the trash if possible.
    /// Parent folders are created on the file if needed.
    pub fn event_trash_restore_file(status: &mut Status) -> FmResult<()> {
        status.trash.restore()?;
        status.selected().mode = Mode::Normal;
        status.selected().refresh_view()?;
        Ok(())
    }

    /// Ask the user if he wants to empty the trash.
    /// It requires a confimation before doing anything
    pub fn event_trash_empty(status: &mut Status) -> FmResult<()> {
        status.selected().mode = Mode::NeedConfirmation(NeedConfirmation::EmptyTrash);
        Ok(())
    }

    /// Empty the trash folder permanently.
    pub fn exec_trash_empty(status: &mut Status) -> FmResult<()> {
        status.trash.empty_trash()?;
        status.clear_flags_and_reset_view()?;
        Ok(())
    }

    /// Open the trash.
    /// Displays a navigable content of the trash.
    /// Each item can be restored or deleted.
    /// Each opening refresh the trash content.
    pub fn event_trash_open(status: &mut Status) -> FmResult<()> {
        status.trash.update()?;
        status.selected().mode = Mode::Navigate(Navigate::Trash);
        Ok(())
    }

    /// Select the next element in the trash folder
    pub fn event_trash_next(status: &mut Status) {
        status.trash.next();
    }

    /// Select the previous element in the trash folder
    pub fn event_trash_prev(status: &mut Status) {
        status.trash.prev();
    }

    /// Remove the selected element in the trash folder.
    pub fn event_trash_remove_file(status: &mut Status) -> FmResult<()> {
        status.trash.remove()
    }

    pub fn event_tree(status: &mut Status) -> FmResult<()> {
        status.make_tree()?;
        status.selected().mode = Mode::Tree;
        let len = status.selected_non_mut().directory.len();
        status.selected().window.reset(len);
        Ok(())
    }

    pub fn event_tree_go_to_root(status: &mut Status) -> FmResult<()> {
        let colors = status.config_colors.clone();
        status.selected().tree_select_root(&colors)
    }

    pub fn event_select_next_sibling(status: &mut Status) -> FmResult<()> {
        let colors = status.config_colors.clone();
        status.selected().tree_select_next_sibling(&colors)
    }

    pub fn event_select_first_child(status: &mut Status) -> FmResult<()> {
        let colors = status.config_colors.clone();
        status.selected().tree_select_first_child(&colors)
    }

    pub fn event_select_parent(status: &mut Status) -> FmResult<()> {
        let colors = status.config_colors.clone();
        status.selected().tree_select_parent(&colors)
    }

    pub fn event_select_prev_sibling(status: &mut Status) -> FmResult<()> {
        let colors = status.config_colors.clone();
        status.selected().tree_select_prev_sibling(&colors)
    }

    pub fn event_tree_go_to_bottom_leaf(status: &mut Status) -> FmResult<()> {
        let colors = status.config_colors.clone();
        status.selected().tree_go_to_bottom_leaf(&colors)
    }

    pub fn exec_tree(tab: &mut Tab) -> FmResult<()> {
        let path = tab.directory.tree.current_path.clone();
        if !path.is_dir() {
            let filename = path
                .file_name()
                .ok_or_else(|| FmError::custom("exec_tree", "path should have a filename"))?
                .to_str()
                .ok_or_else(|| FmError::custom("exec_tree", "path should have a filename"))?;
            let parent = path
                .parent()
                .ok_or_else(|| FmError::custom("exec_tree", "path should have a parent"))?
                .to_owned();
            tab.set_pathcontent(&parent)?;
            tab.mode = Mode::Normal;
            tab.refresh_view()?;
            tab.search_from(filename, 0);
            Ok(())
        } else {
            tab.set_pathcontent(&path)?;
            tab.mode = Mode::Normal;
            tab.refresh_view()
        }
    }
}

fn string_to_path(path_string: &str) -> FmResult<path::PathBuf> {
    let expanded_cow_path = shellexpand::tilde(&path_string);
    let expanded_target: &str = expanded_cow_path.borrow();
    Ok(std::fs::canonicalize(expanded_target)?)
}
