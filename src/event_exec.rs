use std::borrow::Borrow;
use std::cmp::min;
use std::fs;
use std::path;

use copypasta::{ClipboardContext, ClipboardProvider};
use log::info;
use sysinfo::SystemExt;

use crate::bulkrename::Bulkrename;
use crate::completion::InputCompleted;
use crate::compress::{
    compressed_deflate, compressed_gzip, compressed_zip, compressed_zlib, CompressionMethod,
};
use crate::config::Colors;
use crate::constant_strings_paths::CONFIG_PATH;
use crate::constant_strings_paths::DEFAULT_DRAGNDROP;
use crate::constant_strings_paths::NVIM_RPC_SENDER;
use crate::content_window::RESERVED_ROWS;
use crate::copy_move::CopyMove;
use crate::cryptsetup::EncryptedAction;
use crate::cryptsetup::PasswordKind;
use crate::fileinfo::FileKind;
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::git::git_root;
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
    pub fn refresh_status(status: &mut Status, colors: &Colors) -> FmResult<()> {
        status.refresh_users()?;
        status.selected().refresh_view()?;
        if let Mode::Tree = status.selected_non_mut().mode {
            status.selected().make_tree(colors)?
        }
        Ok(())
    }

    /// When a rezise event occurs, we may hide the second panel if the width
    /// isn't sufficiant to display enough information.
    /// We also need to know the new height of the terminal to start scrolling
    /// up or down.
    pub fn resize(
        status: &mut Status,
        width: usize,
        height: usize,
        colors: &Colors,
    ) -> FmResult<()> {
        if width < MIN_WIDTH_FOR_DUAL_PANE {
            status.select_tab(0)?;
            status.set_dual_pane(false);
        } else {
            status.set_dual_pane(true);
        }
        status.selected().set_height(height);
        Self::refresh_status(status, colors)?;
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
        let tab = status.selected_non_mut();

        match tab.mode {
            Mode::Normal => {
                let Some(file) = tab.path_content.selected() else { return Ok(()) };
                let path = file.path.clone();
                status.toggle_flag_on_path(&path);
                Self::event_down_one_row(status.selected());
            }
            Mode::Tree => {
                let path = tab.directory.tree.current_node.filepath();
                status.toggle_flag_on_path(&path);
            }
            _ => (),
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
        status
            .selected()
            .set_mode(Mode::InputSimple(InputSimple::Chmod));
        if status.flagged.is_empty() {
            status
                .flagged
                .push(status.tabs[status.index].selected().unwrap().path.clone());
        };
        status.reset_tabs_view()
    }

    /// Enter JUMP mode, allowing to jump to any flagged file.
    /// Does nothing if no file is flagged.
    pub fn event_jump(status: &mut Status) -> FmResult<()> {
        if !status.flagged.is_empty() {
            status.flagged.index = 0;
            status.selected().set_mode(Mode::Navigate(Navigate::Jump))
        }
        Ok(())
    }

    /// Enter Marks new mode, allowing to bind a char to a path.
    pub fn event_marks_new(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::Navigate(Navigate::Marks(MarkAction::New)));
        Ok(())
    }

    /// Enter Marks jump mode, allowing to jump to a marked file.
    pub fn event_marks_jump(status: &mut Status) -> FmResult<()> {
        if status.marks.is_empty() {
            return Ok(());
        }
        status
            .selected()
            .set_mode(Mode::Navigate(Navigate::Marks(MarkAction::Jump)));
        Ok(())
    }

    /// Jump to the current mark.
    pub fn exec_marks_jump_selected(status: &mut Status) -> FmResult<()> {
        let marks = status.marks.clone();
        let tab = status.selected();
        if let Some((_, path)) = marks.selected() {
            tab.history.push(path);
            tab.set_pathcontent(path)?;
            tab.window.reset(tab.path_content.content.len());
            tab.input.reset();
        }
        Ok(())
    }

    /// Update the selected mark with the current path.
    /// Doesn't change its char.
    /// If it doesn't fail, a new pair will be set with (oldchar, new path).
    pub fn exec_marks_update_selected(status: &mut Status) -> FmResult<()> {
        let marks = status.marks.clone();
        let len = status.selected_non_mut().path_content.content.len();
        if let Some((ch, _)) = marks.selected() {
            if let Some(path_str) = status.selected_non_mut().path_content_str() {
                status.marks.new_mark(*ch, path::PathBuf::from(path_str))?;
            }
            status.selected().window.reset(len);
            status.selected().input.reset();
        }
        Ok(())
    }

    /// Execute a new mark, saving it to a config file for futher use.
    pub fn exec_marks_new(status: &mut Status, c: char, colors: &Colors) -> FmResult<()> {
        let path = status.selected().path_content.path.clone();
        status.marks.new_mark(c, path)?;
        Self::event_normal(status.selected())?;
        status.selected().reset_mode();
        Self::refresh_status(status, colors)
    }

    /// Execute a jump to a mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    pub fn exec_marks_jump(status: &mut Status, c: char, colors: &Colors) -> FmResult<()> {
        if let Some(path) = status.marks.get(c) {
            status.selected().set_pathcontent(&path)?
        }
        Self::event_normal(status.selected())?;
        status.selected().reset_mode();
        Self::refresh_status(status, colors)
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn event_symlink(status: &mut Status) -> FmResult<()> {
        for oldpath in status.flagged.content.iter() {
            let filename = oldpath
                .as_path()
                .file_name()
                .ok_or_else(|| FmError::custom("event symlink", "File not found"))?;
            let newpath = status
                .selected_non_mut()
                .directory_of_selected()?
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
    pub fn exec_delete_files(status: &mut Status, colors: &Colors) -> FmResult<()> {
        for pathbuf in status.flagged.content.iter() {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(pathbuf)?;
            } else {
                std::fs::remove_file(pathbuf)?;
            }
        }
        status.selected().reset_mode();
        status.clear_flags_and_reset_view()?;
        Self::refresh_status(status, colors)
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
        let Some(jump_target) = status.flagged.selected() else { return Ok(()) };
        let jump_target = jump_target.to_owned();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent,
            None => &jump_target,
        };
        let tab = status.selected();
        tab.input.clear();
        tab.history.push(target_dir);
        tab.path_content
            .change_directory(target_dir, &tab.filter, tab.show_hidden)?;
        let index = tab.find_jump_index(&jump_target).unwrap_or_default();
        tab.path_content.select_index(index);
        tab.set_window();
        tab.scroll_to(index);

        Ok(())
    }

    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    pub fn exec_confirmed_action(
        status: &mut Status,
        confirmed_action: NeedConfirmation,
        colors: &Colors,
    ) -> FmResult<()> {
        Self::_exec_confirmed_action(status, confirmed_action, colors)?;
        status.selected().set_mode(Mode::Normal);
        Ok(())
    }

    fn _exec_confirmed_action(
        status: &mut Status,
        confirmed_action: NeedConfirmation,
        colors: &Colors,
    ) -> FmResult<()> {
        match confirmed_action {
            NeedConfirmation::Delete => Self::exec_delete_files(status, colors),
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
    pub fn event_reset_mode(tab: &mut Tab) -> FmResult<()> {
        tab.reset_mode();
        tab.refresh_view()
    }

    /// Reset the inputs and completion, reset the window, exit the preview.
    pub fn event_normal(tab: &mut Tab) -> FmResult<()> {
        tab.refresh_view()
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
    pub fn page_up(tab: &mut Tab) {
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
    pub fn page_down(tab: &mut Tab) {
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

    /// Select the left or right tab depending on where the user clicked.
    pub fn event_select_pane(status: &mut Status, col: u16) -> FmResult<()> {
        let (width, _) = status.term_size()?;
        if (col as usize) < width / 2 {
            status.select_tab(0)?;
        } else {
            status.select_tab(1)?;
        };
        Ok(())
    }

    /// Select a given row, if there's something in it.
    pub fn event_select_row(status: &mut Status, row: u16, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        match tab.mode {
            Mode::Normal => {
                let index = Self::row_to_index(row);
                tab.path_content.select_index(index);
                tab.window.scroll_to(index);
            }
            Mode::Tree => {
                let index = Self::row_to_index(row) + 1;
                tab.directory.tree.unselect_children();
                tab.directory.tree.position = tab.directory.tree.position_from_index(index);
                let (_, _, node) = tab.directory.tree.select_from_position()?;
                tab.directory.make_preview(colors);
                tab.directory.tree.current_node = node;
            }
            _ => (),
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

    /// Select the next shortcut.
    pub fn event_marks_next(status: &mut Status) {
        status.marks.next()
    }

    /// Select the previous shortcut.
    pub fn event_marks_prev(status: &mut Status) {
        status.marks.prev()
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
        tab.move_to_parent()
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
        status
            .selected()
            .set_mode(Mode::NeedConfirmation(NeedConfirmation::Copy));
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
        status
            .selected()
            .set_mode(Mode::NeedConfirmation(NeedConfirmation::Move));
        Ok(())
    }

    /// Enter the new dir mode.
    pub fn event_new_dir(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::InputSimple(InputSimple::Newdir));
        Ok(())
    }

    /// Enter the new file mode.
    pub fn event_new_file(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::InputSimple(InputSimple::Newfile));
        Ok(())
    }

    /// Enter the execute mode. Most commands must be executed to allow for
    /// a confirmation.
    pub fn event_exec(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::InputCompleted(InputCompleted::Exec));
        Ok(())
    }

    /// Preview the selected file.
    /// Every file can be previewed. See the `crate::enum::Preview` for
    /// more details on previewinga file.
    /// Does nothing if the directory is empty.
    pub fn event_preview(status: &mut Status, colors: &Colors) -> FmResult<()> {
        if status.selected_non_mut().path_content.is_empty() {
            return Ok(());
        }
        let unmutable_tab = status.selected_non_mut();
        let Some(file_info) = unmutable_tab.selected() else { return Ok(()) };
        match file_info.file_kind {
            FileKind::NormalFile => {
                let preview = Preview::new(
                    file_info,
                    &unmutable_tab.path_content.users_cache,
                    status,
                    colors,
                )?;
                status.selected().set_mode(Mode::Preview);
                status.selected().window.reset(preview.len());
                status.selected().preview = preview;
            }
            FileKind::Directory => Self::event_tree(status, colors)?,
            _ => (),
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
        status
            .selected()
            .set_mode(Mode::NeedConfirmation(NeedConfirmation::Delete));
        Ok(())
    }

    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn event_help(status: &mut Status) -> FmResult<()> {
        let help = status.help.clone();
        let tab = status.selected();
        tab.set_mode(Mode::Preview);
        tab.preview = Preview::help(&help);
        tab.window.reset(tab.preview.len());
        Ok(())
    }

    /// Enter the search mode.
    /// Matching items are displayed as you type them.
    pub fn event_search(tab: &mut Tab) -> FmResult<()> {
        tab.searched = None;
        tab.set_mode(Mode::InputCompleted(InputCompleted::Search));
        Ok(())
    }

    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    pub fn event_regex_match(tab: &mut Tab) -> FmResult<()> {
        match tab.mode {
            Mode::Tree => (),
            _ => tab.set_mode(Mode::InputSimple(InputSimple::RegexMatch)),
        }
        Ok(())
    }

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn event_sort(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::InputSimple(InputSimple::Sort));
        Ok(())
    }

    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's usefull to reset the cursor before leaving the application.
    pub fn event_quit(tab: &mut Tab) -> FmResult<()> {
        if let Mode::Tree = tab.mode {
            Self::event_normal(tab)?;
            tab.set_mode(Mode::Normal)
        } else {
            tab.must_quit = true;
        }
        Ok(())
    }

    /// Leave a mode requiring a confirmation without doing anything.
    /// Reset the mode to the previous mode.
    pub fn event_leave_need_confirmation(tab: &mut Tab) {
        tab.reset_mode();
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
    pub fn event_leave_sort(status: &mut Status, c: char, colors: &Colors) -> FmResult<()> {
        if status.selected_non_mut().path_content.content.is_empty() {
            return Ok(());
        }
        let tab = status.selected();
        tab.reset_mode();
        match tab.mode {
            Mode::Normal => {
                tab.path_content.unselect_current();
                tab.path_content.update_sort_from_char(c);
                tab.path_content.sort();
                Self::event_go_top(tab);
                tab.path_content.select_index(0);
            }
            Mode::Tree => {
                tab.directory.tree.update_sort_from_char(c);
                tab.directory.tree.sort();
                tab.tree_select_root(colors)?;
                tab.directory.tree.into_navigable_content(colors);
            }
            _ => (),
        }
        Ok(())
    }

    /// Insert a char in the input string.
    pub fn event_text_insertion(tab: &mut Tab, c: char) {
        tab.input.insert(c);
    }

    /// Toggle the display of hidden files.
    pub fn event_toggle_hidden(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        tab.show_hidden = !tab.show_hidden;
        tab.path_content.reset_files(&tab.filter, tab.show_hidden)?;
        tab.window.reset(tab.path_content.content.len());
        if let Mode::Tree = tab.mode {
            tab.make_tree(colors)?
        }
        Ok(())
    }

    /// Open a file with custom opener.
    pub fn event_open_file(status: &mut Status) -> FmResult<()> {
        match status.opener.open(
            &status
                .selected_non_mut()
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
    /// Keep a track of the current mode to ensure we rename the correct file.
    /// When we enter rename from a "tree" mode, we'll need to rename the selected file in the tree,
    /// not the selected file in the pathcontent.
    pub fn event_rename(tab: &mut Tab) -> FmResult<()> {
        if tab.selected().is_some() {
            tab.set_mode(Mode::InputSimple(InputSimple::Rename));
        }
        Ok(())
    }

    /// Enter the goto mode where an user can type a path to jump to.
    pub fn event_goto(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::InputCompleted(InputCompleted::Goto));
        tab.completion.reset();
        Ok(())
    }

    /// Open a new terminal in current directory.
    /// The shell is a fork of current process and will exit if the application
    /// is terminated first.
    pub fn event_shell(status: &mut Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        let path = tab
            .directory_of_selected()?
            .to_str()
            .ok_or_else(|| FmError::custom("event_shell", "Couldn't parse the directory"))?;
        execute_in_child(&status.opener.terminal.clone(), &vec!["-d", path])?;
        Ok(())
    }

    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    pub fn event_history(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::Navigate(Navigate::History));
        Ok(())
    }

    /// Enter the shortcut mode, allowing to visit predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    pub fn event_shortcut(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::Navigate(Navigate::Shortcut));
        Ok(())
    }

    /// A right click opens a file or a directory.
    pub fn event_right_click(status: &mut Status, colors: &Colors) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => Self::exec_file(status),
            Mode::Tree => Self::exec_tree(status, colors),
            _ => Ok(()),
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
        if let Ok(nvim_listen_address) = Self::nvim_listen_address(tab) {
            let Some(fileinfo) = tab.selected() else { return Ok(()) };
            let Some(path_str) = fileinfo.path.to_str() else { return Ok(()) };
            let _ = execute_in_child(
                NVIM_RPC_SENDER,
                &vec![
                    "--remote-send",
                    &format!("<esc>:e {path_str}<cr><esc>:close<cr>"),
                    "--servername",
                    &nvim_listen_address,
                ],
            );
        }
        Ok(())
    }

    /// Copy the selected filename to the clipboard. Only the filename.
    pub fn event_filename_to_clipboard(tab: &Tab) -> FmResult<()> {
        Self::set_clipboard(
            tab.selected()
                .ok_or_else(|| FmError::custom("event_filename_to_clipboard", "no selected file"))?
                .filename
                .clone(),
        )
    }

    /// Copy the selected filepath to the clipboard. The absolute path.
    pub fn event_filepath_to_clipboard(tab: &Tab) -> FmResult<()> {
        Self::set_clipboard(
            tab.selected()
                .ok_or_else(|| FmError::custom("event_filepath_to_clipboard", "no selected file"))?
                .path
                .to_str()
                .ok_or_else(|| FmError::custom("event_filepath_to_clipboard", "no selected file"))?
                .to_owned(),
        )
    }

    fn set_clipboard(content: String) -> FmResult<()> {
        info!("copied to clipboard: {}", content);
        let mut ctx = ClipboardContext::new()?;
        ctx.set_contents(content)?;
        // For some reason, it's not writen if you don't read it back...
        let _ = ctx.get_contents();
        Ok(())
    }

    /// Enter the filter mode, where you can filter.
    /// See `crate::filter::Filter` for more details.
    pub fn event_filter(tab: &mut Tab) -> FmResult<()> {
        tab.set_mode(Mode::InputSimple(InputSimple::Filter));
        Ok(())
    }

    /// Move back in history to the last visited directory.
    pub fn event_back(status: &mut Status, colors: &Colors) -> FmResult<()> {
        if status.selected_non_mut().history.content.len() <= 1 {
            return Ok(());
        }
        let tab = status.selected();
        tab.history.content.pop();
        let last_index = tab.history.len() - 1;
        let last = tab.history.content[last_index].clone();
        tab.set_pathcontent(&last)?;
        if let Mode::Tree = tab.mode {
            tab.make_tree(colors)?
        }

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
    /// We only try to rename in the same directory, so it shouldn't be a problem.
    /// Filename is sanitized before processing.
    pub fn exec_rename(tab: &mut Tab) -> FmResult<()> {
        let fileinfo = match tab.previous_mode {
            Mode::Tree => &tab.directory.tree.current_node.fileinfo,
            _ => tab
                .path_content
                .selected()
                .ok_or_else(|| FmError::custom("rename", "couldnt parse selected"))?,
        };

        let original_path = &fileinfo.path;
        if let Some(parent) = original_path.parent() {
            let new_path = parent.join(sanitize_filename::sanitize(tab.input.string()));
            info!(
                "renaming: original: {} - new: {}",
                original_path.display(),
                new_path.display()
            );
            fs::rename(original_path, new_path)?;
        }

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
            let path = &tab
                .selected()
                .ok_or_else(|| {
                    FmError::custom("exec exec", &format!("can't find command {command}"))
                })?
                .path
                .to_str()
                .ok_or_else(|| {
                    FmError::custom("exec exec", &format!("can't find command {command}"))
                })?;
            // let path = &tab.path_content.selected_path_string().ok_or_else(|| {
            //     FmError::custom("exec exec", &format!("can't find command {}", command))
            // })?;
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
        let Some(file) = tab.selected() else { return Ok(()) };
        let path_str = file
            .path
            .to_str()
            .ok_or_else(|| FmError::custom("event drag n drop", "Couldn't read path"))?;

        execute_in_child(DEFAULT_DRAGNDROP, &vec![path_str])?;
        Ok(())
    }

    /// Executes a search in current folder, selecting the first file matching
    /// the current completion proposition.
    /// ie. If you typed `"jpg"` before, it will move to the first file
    /// whose filename contains `"jpg"`.
    /// The current order of files is used.
    pub fn exec_search(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        let searched = tab.input.string();
        tab.input.reset();
        if searched.is_empty() {
            tab.searched = None;
            return Ok(());
        }
        tab.searched = Some(searched.clone());
        match tab.previous_mode {
            Mode::Tree => {
                tab.directory.tree.unselect_children();
                if let Some(position) = tab.directory.tree.select_first_match(&searched) {
                    tab.directory.tree.position = position;
                    (_, _, tab.directory.tree.current_node) =
                        tab.directory.tree.select_from_position()?;
                } else {
                    tab.directory.tree.select_root()
                };
                tab.directory.make_preview(colors);
                Ok(())
            }
            _ => {
                let next_index = tab.path_content.index;
                tab.search_from(&searched, next_index);
                Ok(())
            }
        }
    }

    pub fn event_search_next(tab: &mut Tab) -> FmResult<()> {
        match tab.mode {
            Mode::Tree => (),
            _ => {
                let Some(searched) = tab.searched.clone() else { return Ok(()) };
                let next_index = (tab.path_content.index + 1) % tab.path_content.content.len();
                tab.search_from(&searched, next_index);
            }
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
    pub fn exec_filter(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        let filter = FilterKind::from_input(&tab.input.string());
        tab.set_filter(filter);
        tab.input.reset();
        tab.path_content.reset_files(&tab.filter, tab.show_hidden)?;
        if let Mode::Tree = tab.previous_mode {
            tab.make_tree(colors)?;
        }
        tab.window.reset(tab.path_content.content.len());
        Ok(())
    }

    /// Move up one row in modes allowing movement.
    /// Does nothing if the selected item is already the first in list.
    pub fn event_move_up(status: &mut Status, colors: &Colors) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_up_one_row(status.selected()),
            Mode::Navigate(Navigate::Jump) => EventExec::event_jumplist_prev(status),
            Mode::Navigate(Navigate::History) => EventExec::event_history_prev(status.selected()),
            Mode::Navigate(Navigate::Trash) => EventExec::event_trash_prev(status),
            Mode::Navigate(Navigate::Shortcut) => EventExec::event_shortcut_prev(status.selected()),
            Mode::Navigate(Navigate::Marks(_)) => EventExec::event_marks_prev(status),
            Mode::Navigate(Navigate::Compress) => EventExec::event_compression_prev(status),
            Mode::Navigate(Navigate::EncryptedDrive) => {
                EventExec::event_encrypted_drive_prev(status)
            }
            Mode::Tree => EventExec::event_select_prev(status.selected(), colors)?,
            Mode::InputCompleted(_) => {
                status.selected().completion.prev();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move down one row in modes allowing movements.
    /// Does nothing if the user is already at the bottom.
    pub fn event_move_down(status: &mut Status, colors: &Colors) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::event_down_one_row(status.selected()),
            Mode::Navigate(Navigate::Jump) => EventExec::event_jumplist_next(status),
            Mode::Navigate(Navigate::History) => EventExec::event_history_next(status.selected()),
            Mode::Navigate(Navigate::Trash) => EventExec::event_trash_next(status),
            Mode::Navigate(Navigate::Shortcut) => EventExec::event_shortcut_next(status.selected()),
            Mode::Navigate(Navigate::Marks(_)) => EventExec::event_marks_next(status),
            Mode::Navigate(Navigate::Compress) => EventExec::event_compression_next(status),
            Mode::Navigate(Navigate::EncryptedDrive) => {
                EventExec::event_encrypted_drive_next(status)
            }
            Mode::InputCompleted(_) => status.selected().completion.next(),
            Mode::Tree => EventExec::event_select_next(status.selected(), colors)?,
            _ => (),
        };
        Ok(())
    }

    /// Move to parent in normal mode,
    /// move left one char in mode requiring text input.
    pub fn event_move_left(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        match tab.mode {
            Mode::Normal => EventExec::event_move_to_parent(tab),
            Mode::Tree => EventExec::event_select_parent(tab, colors),
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
                EventExec::event_move_cursor_left(tab);
                Ok(())
            }

            _ => Ok(()),
        }
    }

    /// Move to child if any or open a regular file in normal mode.
    /// Move the cursor one char to right in mode requiring text input.
    pub fn event_move_right(status: &mut Status, colors: &Colors) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => EventExec::exec_file(status),
            Mode::Tree => EventExec::event_select_first_child(status.selected(), colors),
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
    pub fn event_key_home(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        match tab.mode {
            Mode::Normal | Mode::Preview => EventExec::event_go_top(tab),
            Mode::Tree => EventExec::event_tree_go_to_root(tab, colors)?,
            _ => EventExec::event_cursor_home(tab),
        };
        Ok(())
    }

    /// Move to the bottom in any mode.
    pub fn event_end(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        match tab.mode {
            Mode::Normal | Mode::Preview => EventExec::event_go_bottom(tab),
            Mode::Tree => EventExec::event_tree_go_to_bottom_leaf(tab, colors)?,
            _ => EventExec::event_cursor_end(tab),
        };
        Ok(())
    }

    /// Move up 10 lines in normal mode and preview.
    pub fn event_page_up(status: &mut Status, colors: &Colors) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::page_up(status.selected()),
            Mode::Tree => EventExec::event_tree_page_up(status.selected(), colors)?,
            _ => (),
        };
        Ok(())
    }

    /// Move down 10 lines in normal & preview mode.
    pub fn event_page_down(status: &mut Status, colors: &Colors) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview => EventExec::page_down(status.selected()),
            Mode::Tree => EventExec::event_tree_page_down(status.selected(), colors)?,
            _ => (),
        };
        Ok(())
    }

    /// Execute the mode.
    /// In modes requiring confirmation or text input, it will execute the
    /// related action.
    /// In normal mode, it will open the file.
    /// Reset to normal mode afterwards.
    pub fn event_enter(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let mut must_refresh = true;
        let mut must_reset_mode = true;
        match status.selected_non_mut().mode {
            Mode::InputSimple(InputSimple::Rename) => EventExec::exec_rename(status.selected())?,
            Mode::InputSimple(InputSimple::Newfile) => EventExec::exec_newfile(status.selected())?,
            Mode::InputSimple(InputSimple::Newdir) => EventExec::exec_newdir(status.selected())?,
            Mode::InputSimple(InputSimple::Chmod) => EventExec::exec_chmod(status)?,
            Mode::InputSimple(InputSimple::RegexMatch) => EventExec::exec_regex(status)?,
            Mode::InputSimple(InputSimple::Filter) => {
                must_refresh = false;
                EventExec::exec_filter(status, colors)?
            }
            Mode::InputSimple(InputSimple::Password(password_kind, encrypted_action)) => {
                must_refresh = false;
                must_reset_mode = false;
                EventExec::exec_store_password(status, password_kind)?;
                match encrypted_action {
                    EncryptedAction::MOUNT => EventExec::event_mount_encrypted_drive(status)?,
                    EncryptedAction::UMOUNT => EventExec::event_umount_encrypted_drive(status)?,
                }
            }
            Mode::Navigate(Navigate::Jump) => EventExec::exec_jump(status)?,
            Mode::Navigate(Navigate::History) => EventExec::exec_history(status.selected())?,
            Mode::Navigate(Navigate::Shortcut) => EventExec::exec_shortcut(status.selected())?,
            Mode::Navigate(Navigate::Trash) => EventExec::event_trash_restore_file(status)?,
            Mode::Navigate(Navigate::EncryptedDrive) => (),
            Mode::InputCompleted(InputCompleted::Exec) => EventExec::exec_exec(status.selected())?,
            Mode::InputCompleted(InputCompleted::Search) => {
                must_refresh = false;
                EventExec::exec_search(status, colors)?
            }
            Mode::InputCompleted(InputCompleted::Goto) => EventExec::exec_goto(status.selected())?,
            Mode::Normal => EventExec::exec_file(status)?,
            Mode::Tree => EventExec::exec_tree(status, colors)?,
            Mode::NeedConfirmation(_)
            | Mode::Preview
            | Mode::InputCompleted(InputCompleted::Nothing)
            | Mode::InputSimple(InputSimple::Sort) => (),
            Mode::Navigate(Navigate::Marks(MarkAction::New)) => {
                EventExec::exec_marks_update_selected(status)?
            }
            Mode::Navigate(Navigate::Marks(MarkAction::Jump)) => {
                EventExec::exec_marks_jump_selected(status)?
            }
            Mode::Navigate(Navigate::Compress) => EventExec::exec_compress(status)?,
        };

        status.selected().input.reset();
        if must_reset_mode {
            status.selected().reset_mode();
        }
        if must_refresh {
            Self::refresh_status(status, colors)?;
        }
        Ok(())
    }

    /// Change tab in normal mode with dual pane displayed,
    /// insert a completion in modes allowing completion.
    pub fn event_tab(status: &mut Status) -> FmResult<()> {
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
        if let Mode::Normal | Mode::Tree = status.selected_non_mut().mode {
            return EventExec::event_filename_to_clipboard(status.selected());
        }
        Ok(())
    }

    /// Copy the filepath of the selected file in normal mode.
    pub fn event_copy_filepath(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal | Mode::Tree = status.selected_non_mut().mode {
            return EventExec::event_filepath_to_clipboard(status.selected());
        }
        Ok(())
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn event_refreshview(status: &mut Status, colors: &Colors) -> FmResult<()> {
        status.encrypted_devices.update()?;
        Self::refresh_status(status, colors)
    }

    /// Open a thumbnail of an image, scaled up to the whole window.
    pub fn event_thumbnail(tab: &mut Tab) -> FmResult<()> {
        if let Mode::Normal | Mode::Tree = tab.mode {
            let Some(file_info) = tab.selected() else { return Ok(())};
            info!("selected {:?}", file_info);
            tab.preview = Preview::thumbnail(file_info.path.to_owned())?;
            tab.window.reset(tab.preview.len());
            tab.set_mode(Mode::Preview);
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
        status.selected().reset_mode();
        status.selected().refresh_view()?;
        Ok(())
    }

    /// Ask the user if he wants to empty the trash.
    /// It requires a confimation before doing anything
    pub fn event_trash_empty(status: &mut Status) -> FmResult<()> {
        status
            .selected()
            .set_mode(Mode::NeedConfirmation(NeedConfirmation::EmptyTrash));
        Ok(())
    }

    /// Empty the trash folder permanently.
    pub fn exec_trash_empty(status: &mut Status) -> FmResult<()> {
        status.trash.empty_trash()?;
        status.selected().reset_mode();
        status.clear_flags_and_reset_view()?;
        Ok(())
    }

    /// Open the trash.
    /// Displays a navigable content of the trash.
    /// Each item can be restored or deleted.
    /// Each opening refresh the trash content.
    pub fn event_trash_open(status: &mut Status) -> FmResult<()> {
        status.trash.update()?;
        status.selected().set_mode(Mode::Navigate(Navigate::Trash));
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

    /// Creates a tree in every mode but "Tree".
    /// It tree mode it will exit this view.
    pub fn event_tree(status: &mut Status, colors: &Colors) -> FmResult<()> {
        if let Mode::Tree = status.selected_non_mut().mode {
            Self::event_normal(status.selected())?;
            status.selected().set_mode(Mode::Normal)
        } else {
            status.display_full = true;
            status.selected().make_tree(colors)?;
            status.selected().set_mode(Mode::Tree);
            let len = status.selected_non_mut().directory.len();
            status.selected().window.reset(len);
        }
        Ok(())
    }

    /// Fold the current node of the tree.
    /// Has no effect on "file" nodes.
    pub fn event_tree_fold(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        let (tree, _, _) = tab.directory.tree.explore_position(false);
        tree.node.toggle_fold();
        tab.directory.make_preview(colors);
        Self::event_select_next(tab, colors)
    }

    /// Unfold every child node in the tree.
    /// Recursively explore the tree and unfold every node.
    /// Reset the display.
    pub fn event_tree_unfold_all(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.unfold_children();
        tab.directory.make_preview(colors);
        Ok(())
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node.
    /// Reset the display.
    pub fn event_tree_fold_all(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.fold_children();
        tab.directory.make_preview(colors);
        Ok(())
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node. Reset the display. pub fn event_tree_fold_all(status: &mut Status) -> FmResult<()> { let colors = &status.config_colors.clone(); status.selected().directory.tree.fold_children(); status.selected().directory.make_preview(colors); Ok(()) }
    pub fn event_tree_go_to_root(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.reset_required_height();
        tab.tree_select_root(colors)
    }

    /// Select the first child of the current node and reset the display.
    pub fn event_select_first_child(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.increase_required_height();
        tab.tree_select_first_child(colors)
    }

    /// Select the parent of the current node and reset the display.
    /// Move to the parent and reset the tree if we were in the root node.
    pub fn event_select_parent(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.tree_select_parent(colors)
    }

    /// Select the next sibling of the current node.
    pub fn event_select_next(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.increase_required_height();
        tab.tree_select_next(colors)
    }

    /// Select the previous sibling of the current node.
    pub fn event_select_prev(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.decrease_required_height();
        tab.tree_select_prev(colors)
    }

    /// Move up 10 lines in the tree
    pub fn event_tree_page_up(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.decrease_required_height_by_ten();
        tab.tree_page_up(colors)
    }

    /// Move down 10 lines in the tree
    pub fn event_tree_page_down(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.increase_required_height_by_ten();
        tab.tree_page_down(colors)
    }

    /// Select the last leaf of the tree and reset the view.
    pub fn event_tree_go_to_bottom_leaf(tab: &mut Tab, colors: &Colors) -> FmResult<()> {
        tab.directory.tree.set_required_height(usize::MAX);
        tab.tree_go_to_bottom_leaf(colors)
    }

    /// Execute the selected node if it's a file else enter the directory.
    pub fn exec_tree(status: &mut Status, colors: &Colors) -> FmResult<()> {
        let tab = status.selected();
        let node = tab.directory.tree.current_node.clone();
        if !node.is_dir {
            Self::event_open_file(status)
        } else {
            tab.set_pathcontent(&node.filepath())?;
            tab.make_tree(colors)?;
            Ok(())
        }
    }

    /// Enter the encrypted device menu, allowing the user to mount/umount
    /// a luks encrypted device.
    pub fn event_encrypted_drive(status: &mut Status) -> FmResult<()> {
        if status.encrypted_devices.is_empty() {
            status.encrypted_devices.update()?;
        }
        status
            .selected()
            .set_mode(Mode::Navigate(Navigate::EncryptedDrive));
        Ok(())
    }

    /// Mount the selected encrypted device. Will ask first for sudo password and
    /// passphrase.
    /// Those passwords are always dropped immediatly after the commands are run.
    pub fn event_mount_encrypted_drive(status: &mut Status) -> FmResult<()> {
        if !status.encrypted_devices.has_sudo() {
            Self::event_ask_password(status, PasswordKind::SUDO, EncryptedAction::MOUNT)
        } else if !status.encrypted_devices.has_cryptsetup() {
            Self::event_ask_password(status, PasswordKind::CRYPTSETUP, EncryptedAction::MOUNT)
        } else {
            status.encrypted_devices.mount_selected()
        }
    }

    /// Move to the selected crypted device mount point.
    pub fn event_move_to_encrypted_drive(status: &mut Status) -> FmResult<()> {
        let Some(device) = status.encrypted_devices.selected() else { return Ok(()) };
        let Some(mount_point) = device.cryptdevice.mount_point() else { return Ok(())};
        let tab = status.selected();
        let path = path::PathBuf::from(mount_point);
        tab.history.push(&path);
        tab.set_pathcontent(&path)?;
        Self::event_normal(tab)
    }

    /// Unmount the selected device.
    /// Will ask first for a sudo password which is immediatly forgotten.
    pub fn event_umount_encrypted_drive(status: &mut Status) -> FmResult<()> {
        if !status.encrypted_devices.has_sudo() {
            Self::event_ask_password(status, PasswordKind::SUDO, EncryptedAction::UMOUNT)
        } else {
            status.encrypted_devices.umount_selected()
        }
    }

    /// Ask for a password of some kind (sudo or device passphrase).
    pub fn event_ask_password(
        status: &mut Status,
        password_kind: PasswordKind,
        encrypted_action: EncryptedAction,
    ) -> FmResult<()> {
        status
            .selected()
            .set_mode(Mode::InputSimple(InputSimple::Password(
                password_kind,
                encrypted_action,
            )));
        Ok(())
    }

    /// Store a password of some kind (sudo or device passphrase).
    pub fn exec_store_password(status: &mut Status, password_kind: PasswordKind) -> FmResult<()> {
        let password = status.selected_non_mut().input.string();
        status
            .encrypted_devices
            .set_password(password_kind, password);
        status.selected().reset_mode();
        Ok(())
    }

    /// Select the next encrypted device
    pub fn event_encrypted_drive_next(status: &mut Status) {
        status.encrypted_devices.next()
    }

    /// Select the previous encrypted device.
    pub fn event_encrypted_drive_prev(status: &mut Status) {
        status.encrypted_devices.prev()
    }

    /// Open the config file.
    pub fn event_open_config(status: &mut Status) -> FmResult<()> {
        match status.opener.open(&path::PathBuf::from(
            shellexpand::tilde(CONFIG_PATH).to_string(),
        )) {
            Ok(_) => (),
            Err(e) => info!("Error opening {:?}: the config file {}", CONFIG_PATH, e),
        }
        Ok(())
    }

    /// Move to the git root. Does nothing outside of a git repository.
    pub fn event_git_root(tab: &mut Tab) -> FmResult<()> {
        let Ok(git_root) = git_root() else { return Ok(()) };

        let path = &path::PathBuf::from(git_root);

        if path.exists() {
            tab.set_pathcontent(path)?;
            Self::event_normal(tab)?;
            tab.reset_mode();
        }
        Ok(())
    }

    pub fn event_compress(status: &mut Status) -> FmResult<()> {
        status
            .selected()
            .set_mode(Mode::Navigate(Navigate::Compress));
        Ok(())
    }

    pub fn exec_compress(status: &mut Status) -> FmResult<()> {
        let cwd = std::env::current_dir()?;
        let files = status
            .flagged
            .content
            .iter()
            .map(|abs_path| pathdiff::diff_paths(abs_path, &cwd))
            .filter(|p| p.is_some())
            .map(|p| p.unwrap())
            .collect();
        match status.compression.selected() {
            Some(CompressionMethod::DEFLATE) => {
                let archive_name = "archive.tar.gz".to_owned();
                compressed_gzip(archive_name, files)
            }
            Some(CompressionMethod::GZ) => {
                let archive_name = "archive.tar.gz".to_owned();
                compressed_deflate(archive_name, files)
            }
            Some(CompressionMethod::ZLIB) => {
                let archive_name = "archive.tar.xz".to_owned();
                compressed_zlib(archive_name, files)
            }
            Some(CompressionMethod::ZIP) => {
                let archive_name = "archive.zip".to_owned();
                compressed_zip(archive_name, files)
            }
            None => Ok(()),
        }
    }

    pub fn event_compression_prev(status: &mut Status) {
        status.compression.prev()
    }

    pub fn event_compression_next(status: &mut Status) {
        status.compression.next()
    }
}

fn string_to_path(path_string: &str) -> FmResult<path::PathBuf> {
    let expanded_cow_path = shellexpand::tilde(&path_string);
    let expanded_target: &str = expanded_cow_path.borrow();
    Ok(std::fs::canonicalize(expanded_target)?)
}
