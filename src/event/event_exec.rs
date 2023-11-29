use std::borrow::Borrow;
use std::fs;
use std::path;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};

use crate::app::Status;
use crate::app::Tab;
use crate::common::path_to_string;
use crate::common::LAZYGIT;
use crate::common::NCDU;
use crate::common::{is_program_in_path, open_in_current_neovim, string_to_path};
use crate::common::{
    CONFIG_PATH, DEFAULT_DRAGNDROP, DIFF, GIO, MEDIAINFO, NITROGEN, SSHFS_EXECUTABLE,
};
use crate::config::Bindings;
use crate::config::START_FOLDER;
use crate::event::ActionMap;
use crate::io::execute_and_capture_output_with_path;
use crate::io::execute_custom;
use crate::io::read_log;
use crate::io::{
    execute, execute_and_capture_output_without_check, execute_without_output_with_path,
};
use crate::log_info;
use crate::log_line;
use crate::modes::help_string;
use crate::modes::lsblk_and_cryptsetup_installed;
use crate::modes::Display;
use crate::modes::FilterKind;
use crate::modes::InputCompleted;
use crate::modes::Mocp;
use crate::modes::NodeCreation;
use crate::modes::RemovableDevices;
use crate::modes::SelectableContent;
use crate::modes::ShellCommandParser;
use crate::modes::TuiApplications;
use crate::modes::MOCP;
use crate::modes::{Edit, InputSimple, MarkAction, Navigate, NeedConfirmation};
use crate::modes::{ExtensionKind, Preview};

/// Links events from tuikit to custom actions.
/// It mutates `Status` or its children `Tab`.
pub struct EventAction {}

impl EventAction {
    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's usefull to reset the cursor before leaving the application.
    pub fn quit(status: &mut Status) -> Result<()> {
        status.must_quit = true;
        Ok(())
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn refresh_view(status: &mut Status) -> Result<()> {
        status.refresh_view()
    }

    /// Refresh the view if files were modified in current directory.
    pub fn refresh_if_needed(tab: &mut Tab) -> Result<()> {
        tab.refresh_if_needed()
    }

    pub fn resize(status: &mut Status, width: usize, height: usize) -> Result<()> {
        status.resize(width, height)
    }

    /// Leave current mode to normal mode.
    /// Reset the inputs and completion, reset the window, exit the preview.
    pub fn reset_mode(tab: &mut Tab) -> Result<()> {
        if matches!(tab.display_mode, Display::Preview) {
            tab.set_display_mode(Display::Normal);
        }
        if tab.reset_edit_mode() {
            tab.refresh_view()
        } else {
            tab.refresh_params()
        }
    }

    /// Creates a tree in every mode but "Tree".
    /// In display_mode tree it will exit this view.
    pub fn tree(tab: &mut Tab) -> Result<()> {
        tab.toggle_tree_mode()
    }

    /// Fold the current node of the tree.
    /// Has no effect on "file" nodes.
    pub fn tree_fold(tab: &mut Tab) -> Result<()> {
        tab.tree.toggle_fold();
        Ok(())
    }

    /// Unfold every child node in the tree.
    /// Recursively explore the tree and unfold every node.
    /// Reset the display.
    pub fn tree_unfold_all(tab: &mut Tab) -> Result<()> {
        tab.tree.unfold_all();
        Ok(())
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node.
    /// Reset the display.
    pub fn tree_fold_all(tab: &mut Tab) -> Result<()> {
        tab.tree.fold_all();
        Ok(())
    }

    /// Preview the selected file.
    /// Every file can be previewed. See the `crate::enum::Preview` for
    /// more details on previewinga file.
    /// Does nothing if the directory is empty.
    pub fn preview(tab: &mut Tab) -> Result<()> {
        tab.make_preview()
    }

    /// Toggle the display of hidden files.
    pub fn toggle_hidden(tab: &mut Tab) -> Result<()> {
        tab.toggle_hidden()
    }

    /// Remove every flag on files in this directory and others.
    pub fn clear_flags(status: &mut Status) -> Result<()> {
        status.menu.flagged.clear();
        Ok(())
    }

    /// Flag all files in the current directory.
    pub fn flag_all(status: &mut Status) -> Result<()> {
        status.flag_all();
        Ok(())
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn reverse_flags(status: &mut Status) -> Result<()> {
        status.reverse_flags();
        Ok(())
    }

    /// Toggle a single flag and move down one row.
    pub fn toggle_flag(status: &mut Status) -> Result<()> {
        status.toggle_flag_for_selected();
        Ok(())
    }

    /// Enter the rename mode.
    /// Keep a track of the current mode to ensure we rename the correct file.
    /// When we enter rename from a "tree" mode, we'll need to rename the selected file in the tree,
    /// not the selected file in the pathcontent.
    pub fn rename(status: &mut Status) -> Result<()> {
        let selected = status.current_tab().current_file()?;
        if selected.path == status.current_tab().path_content.path {
            return Ok(());
        }
        if let Some(parent) = status.current_tab().path_content.path.parent() {
            if selected.path == parent {
                return Ok(());
            }
        }
        let old_name = &selected.filename;
        status.menu.input.replace(old_name);
        status
            .current_tab_mut()
            .set_edit_mode(Edit::InputSimple(InputSimple::Rename));
        Ok(())
    }

    /// Enter a copy paste mode.
    /// A confirmation is asked before copying all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn copy_paste(status: &mut Status) -> Result<()> {
        Self::set_copy_paste(status, NeedConfirmation::Copy)
    }

    /// Enter the 'move' mode.
    /// A confirmation is asked before moving all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn cut_paste(status: &mut Status) -> Result<()> {
        Self::set_copy_paste(status, NeedConfirmation::Move)
    }

    fn set_copy_paste(status: &mut Status, copy_or_move: NeedConfirmation) -> Result<()> {
        if status.menu.flagged.is_empty() {
            return Ok(());
        }
        status
            .current_tab_mut()
            .set_edit_mode(Edit::NeedConfirmation(copy_or_move));
        Ok(())
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn symlink(status: &mut Status) -> Result<()> {
        for original_file in status.menu.flagged.content.iter() {
            let filename = original_file
                .as_path()
                .file_name()
                .context("event symlink: File not found")?;
            let link = status.current_tab().directory_of_selected()?.join(filename);
            std::os::unix::fs::symlink(original_file, &link)?;
            log_line!(
                "Symlink {link} links to {original_file}",
                original_file = original_file.display(),
                link = link.display()
            );
        }
        status.clear_flags_and_reset_view()
    }

    /// Change to CHMOD mode allowing to edit permissions of a file.
    pub fn chmod(status: &mut Status) -> Result<()> {
        status.set_mode_chmod()
    }

    /// Enter the new dir mode.
    pub fn new_dir(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Newdir));
        Ok(())
    }

    /// Enter the new file mode.
    pub fn new_file(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Newfile));
        Ok(())
    }

    /// Enter the execute mode. Most commands must be executed to allow for
    /// a confirmation.
    pub fn exec(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputCompleted(InputCompleted::Exec));
        Ok(())
    }

    /// Enter the delete mode.
    /// A confirmation is then asked before deleting all the flagged files.
    /// If no file is flagged, flag the selected one before entering the mode.
    pub fn delete_file(status: &mut Status) -> Result<()> {
        if status.menu.flagged.is_empty() {
            Self::toggle_flag(status)?;
        }
        status
            .current_tab_mut()
            .set_edit_mode(Edit::NeedConfirmation(NeedConfirmation::Delete));
        Ok(())
    }

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn sort(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Sort));
        Ok(())
    }

    /// Enter the filter mode, where you can filter.
    /// See `crate::modes::Filter` for more details.
    pub fn filter(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Filter));
        Ok(())
    }

    /// Enter JUMP mode, allowing to jump to any flagged file.
    /// Does nothing if no file is flagged.
    pub fn jump(status: &mut Status) -> Result<()> {
        if !status.menu.flagged.is_empty() {
            status.menu.flagged.index = 0;
            status
                .current_tab_mut()
                .set_edit_mode(Edit::Navigate(Navigate::Jump))
        }
        Ok(())
    }

    /// Enter bulkrename mode, opening a random temp file where the user
    /// can edit the selected filenames.
    /// Once the temp file is saved, those file names are changed.
    pub fn bulk(status: &mut Status) -> Result<()> {
        status.menu.init_bulk();
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::Bulk));
        Ok(())
    }

    /// Enter the search mode.
    /// Matching items are displayed as you type them.
    pub fn search(tab: &mut Tab) -> Result<()> {
        tab.searched = None;
        tab.set_edit_mode(Edit::InputCompleted(InputCompleted::Search));
        Ok(())
    }

    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    pub fn regex_match(tab: &mut Tab) -> Result<()> {
        if !matches!(tab.edit_mode, Edit::Nothing) {
            return Ok(());
        }
        tab.set_edit_mode(Edit::InputSimple(InputSimple::RegexMatch));
        Ok(())
    }

    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn help(status: &mut Status, binds: &Bindings) -> Result<()> {
        let help = help_string(binds, &status.opener)?;
        status.current_tab_mut().set_display_mode(Display::Preview);
        status.current_tab_mut().preview = Preview::help(&help);
        let len = status.current_tab().preview.len();
        status.current_tab_mut().window.reset(len);
        Ok(())
    }

    /// Display the last actions impacting the file tree
    pub fn log(tab: &mut Tab) -> Result<()> {
        let log = read_log()?;
        tab.set_display_mode(Display::Preview);
        tab.preview = Preview::log(log);
        tab.window.reset(tab.preview.len());
        tab.preview_go_bottom();
        Ok(())
    }

    /// Open files with custom opener.
    /// If there's no flagged file, the selected is chosen.
    /// Otherwise, it will open the flagged files (not the flagged directories) with
    /// their respective opener.
    /// Directories aren't opened since it will lead nowhere, it would only replace the
    /// current tab multiple times. It may change in the future.
    /// Only files which use an external opener are supported.
    pub fn open_file(status: &mut Status) -> Result<()> {
        if status.menu.flagged.is_empty() {
            status.open_selected_file()
        } else {
            status.open_flagged_files()
        }
    }

    /// Enter the goto mode where an user can type a path to jump to.
    pub fn goto(status: &mut Status) -> Result<()> {
        status
            .current_tab_mut()
            .set_edit_mode(Edit::InputCompleted(InputCompleted::Goto));
        status.menu.completion.reset();
        Ok(())
    }

    /// Open a new terminal in current directory.
    /// The shell is a fork of current process and will exit if the application
    /// is terminated first.
    pub fn shell(status: &mut Status) -> Result<()> {
        let tab = status.current_tab();
        let path = tab.directory_of_selected()?;
        execute_without_output_with_path(&status.opener.terminal, path, None)?;
        Ok(())
    }

    pub fn shell_command(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Shell));
        Ok(())
    }

    /// Enter the shell menu mode. You can pick a TUI application to be run
    pub fn tui_menu(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::Navigate(Navigate::TuiApplication));
        Ok(())
    }

    /// Enter the cli info mode. You can pick a Text application to be
    /// displayed/
    pub fn cli_menu(status: &mut Status) -> Result<()> {
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::CliApplication));
        Ok(())
    }

    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    pub fn history(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::Navigate(Navigate::History));
        Ok(())
    }

    /// Enter Marks new mode, allowing to bind a char to a path.
    pub fn marks_new(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::Navigate(Navigate::Marks(MarkAction::New)));
        Ok(())
    }

    /// Enter Marks jump mode, allowing to jump to a marked file.
    pub fn marks_jump(status: &mut Status) -> Result<()> {
        if status.menu.marks.is_empty() {
            return Ok(());
        }
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::Marks(MarkAction::Jump)));
        Ok(())
    }

    /// Enter the shortcut mode, allowing to visit predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    pub fn shortcut(status: &mut Status) -> Result<()> {
        std::env::set_current_dir(status.current_tab().directory_of_selected()?)?;
        status.menu.shortcut.update_git_root();
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::Shortcut));
        Ok(())
    }
    /// Send a signal to parent NVIM process, picking files.
    /// If there's no flagged file, it picks the selected one.
    /// otherwise, flagged files are picked.
    /// If no RPC server were provided at launch time - which may happen for
    /// reasons unknow to me - it does nothing.
    /// It requires the "nvim-send" application to be in $PATH.
    pub fn nvim_filepicker(status: &mut Status) -> Result<()> {
        status.update_nvim_listen_address();
        if status.nvim_server.is_empty() {
            return Ok(());
        };
        let nvim_server = status.nvim_server.clone();
        if status.menu.flagged.is_empty() {
            let Ok(fileinfo) = status.current_tab().current_file() else {
                return Ok(());
            };
            open_in_current_neovim(&fileinfo.path, &nvim_server);
        } else {
            let flagged = status.menu.flagged.content.clone();
            for file_path in flagged.iter() {
                open_in_current_neovim(file_path, &nvim_server)
            }
        }

        Ok(())
    }

    pub fn set_nvim_server(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::SetNvimAddr));
        Ok(())
    }

    /// Move back in history to the last visited directory.
    pub fn back(tab: &mut Tab) -> Result<()> {
        tab.back()
    }

    /// Move to $HOME aka ~.
    pub fn home(status: &mut Status) -> Result<()> {
        let home_cow = shellexpand::tilde("~");
        let home: &str = home_cow.borrow();
        let path = std::fs::canonicalize(home)?;
        status.current_tab_mut().cd(&path)?;
        status.update_second_pane_for_preview()
    }

    pub fn go_root(status: &mut Status) -> Result<()> {
        let root_path = std::path::PathBuf::from("/");
        status.current_tab_mut().cd(&root_path)?;
        status.update_second_pane_for_preview()
    }

    pub fn go_start(status: &mut Status) -> Result<()> {
        status.current_tab_mut().cd(&START_FOLDER)?;
        status.update_second_pane_for_preview()
    }

    /// Executes a `dragon-drop` command on the selected file.
    /// It obviously requires the `dragon-drop` command to be installed.
    pub fn drag_n_drop(status: &mut Status) -> Result<()> {
        if !is_program_in_path(DEFAULT_DRAGNDROP) {
            log_line!("{DEFAULT_DRAGNDROP} must be installed.");
            return Ok(());
        }
        let Ok(file) = status.current_tab().current_file() else {
            return Ok(());
        };
        let path_str = file
            .path
            .to_str()
            .context("event drag n drop: couldn't read path")?;

        execute(DEFAULT_DRAGNDROP, &[path_str])?;
        Ok(())
    }

    pub fn search_next(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        let Some(searched) = tab.searched.clone() else {
            return Ok(());
        };
        match tab.display_mode {
            Display::Tree => tab.tree.search_first_match(&searched),
            Display::Normal => tab.normal_search_next(&searched),
            Display::Preview => {
                return Ok(());
            }
        }
        status.set_second_pane_for_preview()?;
        Ok(())
    }

    /// Move up one row in modes allowing movement.
    /// Does nothing if the selected item is already the first in list.
    pub fn move_up(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.edit_mode {
            Edit::Nothing => Self::move_display_up(status)?,
            Edit::Navigate(Navigate::Jump) => status.menu.flagged.prev(),
            Edit::Navigate(Navigate::History) => tab.history.prev(),
            Edit::Navigate(Navigate::Trash) => status.menu.trash.prev(),
            Edit::Navigate(Navigate::Shortcut) => status.menu.shortcut.prev(),
            Edit::Navigate(Navigate::Marks(_)) => status.menu.marks.prev(),
            Edit::Navigate(Navigate::Compress) => status.menu.compression.prev(),
            Edit::Navigate(Navigate::Bulk) => status.menu.bulk_prev(),
            Edit::Navigate(Navigate::TuiApplication) => status.menu.tui_applications.prev(),
            Edit::Navigate(Navigate::CliApplication) => status.menu.cli_applications.prev(),
            Edit::Navigate(Navigate::EncryptedDrive) => status.menu.encrypted_devices.prev(),
            Edit::InputCompleted(_) => status.menu.completion.prev(),
            _ => (),
        };
        status.update_second_pane_for_preview()
    }

    fn move_display_up(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Normal => tab.normal_up_one_row(),
            Display::Preview => tab.preview_page_up(),
            Display::Tree => tab.tree_select_prev()?,
        }
        Ok(())
    }

    fn move_display_down(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Normal => tab.normal_down_one_row(),
            Display::Preview => tab.preview_page_down(),
            Display::Tree => tab.tree_select_next()?,
        }
        Ok(())
    }
    /// Move down one row in modes allowing movements.
    /// Does nothing if the user is already at the bottom.
    pub fn move_down(status: &mut Status) -> Result<()> {
        match status.current_tab_mut().edit_mode {
            Edit::Nothing => Self::move_display_down(status)?,
            Edit::Navigate(Navigate::Jump) => status.menu.flagged.next(),
            Edit::Navigate(Navigate::History) => status.current_tab_mut().history.next(),
            Edit::Navigate(Navigate::Trash) => status.menu.trash.next(),
            Edit::Navigate(Navigate::Shortcut) => status.menu.shortcut.next(),
            Edit::Navigate(Navigate::Marks(_)) => status.menu.marks.next(),
            Edit::Navigate(Navigate::Compress) => status.menu.compression.next(),
            Edit::Navigate(Navigate::Bulk) => status.menu.bulk_next(),
            Edit::Navigate(Navigate::TuiApplication) => status.menu.tui_applications.next(),
            Edit::Navigate(Navigate::CliApplication) => status.menu.cli_applications.next(),
            Edit::Navigate(Navigate::EncryptedDrive) => status.menu.encrypted_devices.next(),
            Edit::InputCompleted(_) => status.menu.completion.next(),
            _ => (),
        };
        status.update_second_pane_for_preview()
    }

    /// Move to parent in normal mode,
    /// move left one char in mode requiring text input.
    pub fn move_left(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.edit_mode {
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
                status.menu.input.cursor_left();
            }
            Edit::Nothing => match tab.display_mode {
                Display::Normal => tab.move_to_parent()?,
                Display::Tree => tab.tree_select_parent()?,
                _ => (),
            },

            _ => (),
        }
        status.update_second_pane_for_preview()
    }

    /// Move to child if any or open a regular file in normal mode.
    /// Move the cursor one char to right in mode requiring text input.
    pub fn move_right(status: &mut Status) -> Result<()> {
        let tab: &mut Tab = status.current_tab_mut();
        match tab.edit_mode {
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
                status.menu.input.cursor_right();
                Ok(())
            }
            Edit::Nothing => match tab.display_mode {
                Display::Normal => LeaveMode::open_file(status),
                Display::Tree => {
                    if tab.tree.selected_path().is_file() {
                        tab.tree_select_next()?;
                    } else {
                        LeaveMode::open_file(status)?;
                    };
                    status.update_second_pane_for_preview()
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }

    /// Delete a char to the left in modes allowing edition.
    pub fn backspace(status: &mut Status) -> Result<()> {
        match status.current_tab().edit_mode {
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
                status.menu.input.delete_char_left();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Delete all chars to the right in mode allowing edition.
    pub fn delete(status: &mut Status) -> Result<()> {
        match status.current_tab_mut().edit_mode {
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
                status.menu.input.delete_chars_right();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Move to leftmost char in mode allowing edition.
    pub fn key_home(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.edit_mode {
            Edit::Nothing => {
                match tab.display_mode {
                    Display::Normal => tab.normal_go_top(),
                    Display::Preview => tab.preview_go_top(),
                    Display::Tree => tab.tree_go_to_root()?,
                };
            }
            _ => status.menu.input.cursor_start(),
        }
        status.update_second_pane_for_preview()
    }

    /// Move to the bottom in any mode.
    pub fn end(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.edit_mode {
            Edit::Nothing => {
                match tab.display_mode {
                    Display::Normal => tab.normal_go_bottom(),
                    Display::Preview => tab.preview_go_bottom(),
                    Display::Tree => tab.tree_go_to_bottom_leaf()?,
                };
            }
            _ => status.menu.input.cursor_end(),
        }
        status.update_second_pane_for_preview()
    }

    /// Move up 10 lines in normal mode and preview.
    pub fn page_up(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Normal => {
                tab.normal_page_up();
                status.update_second_pane_for_preview()?;
            }
            Display::Preview => tab.preview_page_up(),
            Display::Tree => {
                tab.tree_page_up();
                status.update_second_pane_for_preview()?;
            }
        };
        Ok(())
    }

    /// Move down 10 lines in normal & preview mode.
    pub fn page_down(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Normal => {
                tab.normal_page_down();
                status.update_second_pane_for_preview()?;
            }
            Display::Preview => tab.preview_page_down(),
            Display::Tree => {
                tab.tree_page_down()?;
                status.update_second_pane_for_preview()?;
            }
        };
        Ok(())
    }

    /// Execute the mode.
    /// In modes requiring confirmation or text input, it will execute the
    /// related action.
    /// In normal mode, it will open the file.
    /// Reset to normal mode afterwards.
    pub fn enter(status: &mut Status, binds: &Bindings) -> Result<()> {
        let mut must_refresh = true;
        let mut must_reset_mode = true;
        match status.current_tab().edit_mode {
            Edit::InputSimple(InputSimple::Rename) => LeaveMode::rename(status)?,
            Edit::InputSimple(InputSimple::Newfile) => LeaveMode::new_file(status)?,
            Edit::InputSimple(InputSimple::Newdir) => LeaveMode::new_dir(status)?,
            Edit::InputSimple(InputSimple::Chmod) => LeaveMode::chmod(status)?,
            Edit::InputSimple(InputSimple::RegexMatch) => LeaveMode::regex(status)?,
            Edit::InputSimple(InputSimple::SetNvimAddr) => LeaveMode::set_nvim_addr(status)?,
            Edit::InputSimple(InputSimple::Shell) => {
                must_reset_mode = false;
                must_refresh = LeaveMode::shell(status)?;
            }
            Edit::InputSimple(InputSimple::Filter) => {
                must_refresh = false;
                LeaveMode::filter(status)?
            }
            Edit::InputSimple(InputSimple::Password(_, _)) => {
                must_refresh = false;
                must_reset_mode = false;
                LeaveMode::password(status)?
            }
            Edit::InputSimple(InputSimple::Remote) => LeaveMode::remote(status)?,
            Edit::Navigate(Navigate::Jump) => {
                must_refresh = false;
                LeaveMode::jump(status)?
            }
            Edit::Navigate(Navigate::History) => {
                must_refresh = false;
                LeaveMode::history(status)?
            }
            Edit::Navigate(Navigate::Shortcut) => LeaveMode::shortcut(status)?,
            Edit::Navigate(Navigate::Trash) => LeaveMode::trash(status)?,
            Edit::Navigate(Navigate::Bulk) => LeaveMode::bulk(status)?,
            Edit::Navigate(Navigate::TuiApplication) => LeaveMode::shellmenu(status)?,
            Edit::Navigate(Navigate::CliApplication) => {
                must_refresh = false;
                LeaveMode::cli_info(status)?;
            }
            Edit::Navigate(Navigate::EncryptedDrive) => (),
            Edit::Navigate(Navigate::Marks(MarkAction::New)) => LeaveMode::marks_update(status)?,
            Edit::Navigate(Navigate::Marks(MarkAction::Jump)) => LeaveMode::marks_jump(status)?,
            Edit::Navigate(Navigate::Compress) => LeaveMode::compress(status)?,
            Edit::Navigate(Navigate::RemovableDevices) => (),
            Edit::InputCompleted(InputCompleted::Exec) => LeaveMode::exec(status)?,
            Edit::InputCompleted(InputCompleted::Search) => {
                must_refresh = false;
                LeaveMode::search(status)?
            }
            Edit::InputCompleted(InputCompleted::Goto) => LeaveMode::goto(status)?,
            Edit::InputCompleted(InputCompleted::Command) => LeaveMode::command(status, binds)?,
            Edit::NeedConfirmation(_)
            | Edit::InputCompleted(InputCompleted::Nothing)
            | Edit::InputSimple(InputSimple::Sort) => (),
            Edit::Nothing => match status.current_tab().display_mode {
                Display::Normal => {
                    must_refresh = false;
                    must_reset_mode = false;
                    LeaveMode::open_file(status)?;
                }
                Display::Tree => LeaveMode::tree(status)?,
                _ => (),
            },
        };

        status.menu.input.reset();
        if must_reset_mode {
            status.current_tab_mut().reset_edit_mode();
        }
        if must_refresh {
            status.refresh_status()?;
        }
        Ok(())
    }

    /// Change tab in normal mode with dual pane displayed,
    /// insert a completion in modes allowing completion.
    pub fn tab(status: &mut Status) -> Result<()> {
        match status.current_tab_mut().edit_mode {
            Edit::InputCompleted(_) => status
                .menu
                .input
                .replace(status.menu.completion.current_proposition()),
            Edit::Nothing => status.next(),
            _ => (),
        };
        Ok(())
    }

    /// Change tab in normal mode.
    pub fn backtab(status: &mut Status) -> Result<()> {
        if matches!(status.current_tab().edit_mode, Edit::Nothing) {
            status.prev()
        };
        Ok(())
    }

    /// Start a fuzzy find with skim.
    pub fn fuzzyfind(status: &mut Status) -> Result<()> {
        status.skim_output_to_tab()?;
        status.update_second_pane_for_preview()
    }

    /// Start a fuzzy find for a specific line with skim.
    pub fn fuzzyfind_line(status: &mut Status) -> Result<()> {
        status.skim_line_output_to_tab()?;
        status.update_second_pane_for_preview()
    }

    /// Start a fuzzy find for a keybinding with skim.
    pub fn fuzzyfind_help(status: &mut Status, binds: &Bindings) -> Result<()> {
        let help = help_string(binds, &status.opener)?;
        status.skim_find_keybinding_and_run(help)
    }

    /// Copy the filename of the selected file in normal mode.
    pub fn copy_filename(tab: &mut Tab) -> Result<()> {
        if let Display::Normal | Display::Tree = tab.display_mode {
            tab.filename_to_clipboard();
        }
        Ok(())
    }

    /// Copy the filepath of the selected file in normal mode.
    pub fn copy_filepath(tab: &mut Tab) -> Result<()> {
        if let Display::Normal | Display::Tree = tab.display_mode {
            tab.filepath_to_clipboard();
        }
        Ok(())
    }

    /// Display mediainfo details of an image
    pub fn mediainfo(tab: &mut Tab) -> Result<()> {
        if !is_program_in_path(MEDIAINFO) {
            log_line!("{} isn't installed", MEDIAINFO);
            return Ok(());
        }
        if let Display::Normal | Display::Tree = tab.display_mode {
            let Ok(file_info) = tab.current_file() else {
                return Ok(());
            };
            log_info!("selected {:?}", file_info);
            tab.preview = Preview::mediainfo(&file_info.path)?;
            tab.window.reset(tab.preview.len());
            tab.set_display_mode(Display::Preview);
        }
        Ok(())
    }

    /// Display a diff between the first 2 flagged files or dir.
    pub fn diff(status: &mut Status) -> Result<()> {
        if !is_program_in_path(DIFF) {
            log_line!("{DIFF} isn't installed");
            return Ok(());
        }
        if status.menu.flagged.len() < 2 {
            return Ok(());
        };
        if let Display::Normal | Display::Tree = status.current_tab().display_mode {
            let first_path = &status.menu.flagged.content[0]
                .to_str()
                .context("Couldn't parse filename")?;
            let second_path = &status.menu.flagged.content[1]
                .to_str()
                .context("Couldn't parse filename")?;
            status.current_tab_mut().preview = Preview::diff(first_path, second_path)?;
            let tab = status.current_tab_mut();
            tab.window.reset(tab.preview.len());
            tab.set_display_mode(Display::Preview);
        }
        Ok(())
    }

    /// Toggle between a full display (aka ls -lah) or a simple mode (only the
    /// filenames).
    pub fn toggle_display_full(status: &mut Status) -> Result<()> {
        status.settings.metadata = !status.settings.metadata;
        Ok(())
    }

    /// Toggle between dualpane and single pane. Does nothing if the width
    /// is too low to display both panes.
    pub fn toggle_dualpane(status: &mut Status) -> Result<()> {
        status.settings.dual = !status.settings.dual;
        status.select_left();
        Ok(())
    }

    /// Move flagged files to the trash directory.
    /// If no file is flagged, flag the selected file.
    /// More information in the trash crate itself.
    /// If the file is mounted on the $topdir of the trash (aka the $HOME mount point),
    /// it is moved there.
    /// Else, nothing is done.
    pub fn trash_move_file(status: &mut Status) -> Result<()> {
        if status.menu.flagged.is_empty() {
            Self::toggle_flag(status)?;
        }

        status.menu.trash.update()?;
        for flagged in status.menu.flagged.content.iter() {
            status.menu.trash.trash(flagged)?;
        }
        status.menu.flagged.clear();
        status.current_tab_mut().refresh_view()?;
        Ok(())
    }
    /// Ask the user if he wants to empty the trash.
    /// It requires a confimation before doing anything
    pub fn trash_empty(status: &mut Status) -> Result<()> {
        status.menu.trash.update()?;
        status
            .current_tab_mut()
            .set_edit_mode(Edit::NeedConfirmation(NeedConfirmation::EmptyTrash));
        Ok(())
    }

    /// Open the trash.
    /// Displays a navigable content of the trash.
    /// Each item can be restored or deleted.
    /// Each opening refresh the trash content.
    pub fn trash_open(status: &mut Status) -> Result<()> {
        status.menu.trash.update()?;
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::Trash));
        Ok(())
    }

    /// Enter the encrypted device menu, allowing the user to mount/umount
    /// a luks encrypted device.
    pub fn encrypted_drive(status: &mut Status) -> Result<()> {
        if !lsblk_and_cryptsetup_installed() {
            log_line!("lsblk and cryptsetup must be installed.");
            return Ok(());
        }
        if status.menu.encrypted_devices.is_empty() {
            status.menu.encrypted_devices.update()?;
        }
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::EncryptedDrive));
        Ok(())
    }

    pub fn removable_devices(status: &mut Status) -> Result<()> {
        if !is_program_in_path(GIO) {
            log_line!("gio must be installed.");
            return Ok(());
        }
        status.menu.removable_devices = RemovableDevices::from_gio();
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::RemovableDevices));
        Ok(())
    }

    /// Open the config file.
    pub fn open_config(status: &mut Status) -> Result<()> {
        match status.opener.open_single(&path::PathBuf::from(
            shellexpand::tilde(CONFIG_PATH).to_string(),
        )) {
            Ok(_) => (),
            Err(e) => log_info!("Error opening {:?}: the config file {}", CONFIG_PATH, e),
        }
        Ok(())
    }

    /// Enter compression mode
    pub fn compress(status: &mut Status) -> Result<()> {
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::Compress));
        Ok(())
    }

    /// Enter command mode in which you can type any valid command.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    pub fn command(status: &mut Status) -> Result<()> {
        status
            .current_tab_mut()
            .set_edit_mode(Edit::InputCompleted(InputCompleted::Command));
        status.menu.completion.reset();
        Ok(())
    }

    /// Toggle the second pane between preview & normal mode (files).
    pub fn toggle_preview_second(status: &mut Status) -> Result<()> {
        status.settings.preview = !status.settings.preview;
        if status.settings.preview {
            status.set_second_pane_for_preview()?;
        } else {
            status.tabs[1].reset_edit_mode();
            status.tabs[1].refresh_view()?;
        }
        Ok(())
    }

    /// Set the current selected file as wallpaper with `nitrogen`.
    /// Requires `nitrogen` to be installed.
    pub fn set_wallpaper(tab: &Tab) -> Result<()> {
        if !is_program_in_path(NITROGEN) {
            log_line!("nitrogen must be installed");
            return Ok(());
        }
        let Some(fileinfo) = tab.path_content.selected() else {
            return Ok(());
        };
        if !matches!(
            ExtensionKind::matcher(&fileinfo.extension),
            ExtensionKind::Image,
        ) {
            return Ok(());
        }
        let Some(path_str) = tab.path_content.selected_path_string() else {
            return Ok(());
        };
        let _ = execute(NITROGEN, &["--set-zoom-fill", "--save", &path_str]);
        Ok(())
    }

    /// Execute a custom event on the selected file
    pub fn custom(status: &mut Status, string: &String) -> Result<()> {
        log_info!("custom {string}");
        let parser = ShellCommandParser::new(string);
        let mut args = parser.compute(status)?;
        let command = args.remove(0);
        let args: Vec<&str> = args.iter().map(|s| &**s).collect();
        let output = execute_and_capture_output_without_check(command, &args)?;
        log_info!("output {output}");
        Ok(())
    }

    pub fn remote_mount(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Remote));
        Ok(())
    }

    pub fn click_files(status: &mut Status, row: u16, col: u16) -> Result<()> {
        status.click(row, col)
    }

    pub fn select_pane(status: &mut Status, col: u16) -> Result<()> {
        status.select_pane(col)
    }

    pub fn click_first_line(col: u16, status: &mut Status, binds: &Bindings) -> Result<()> {
        status.first_line_action(col, binds)
    }

    pub fn lazygit(status: &mut Status) -> Result<()> {
        Self::open_program(status, LAZYGIT)
    }

    pub fn ncdu(status: &mut Status) -> Result<()> {
        Self::open_program(status, NCDU)
    }

    pub fn open_program(status: &mut Status, program: &str) -> Result<()> {
        if is_program_in_path(program) {
            TuiApplications::require_cwd_and_command(status, program)
        } else {
            Ok(())
        }
    }

    /// Add a song or a folder to MOC playlist. Start it first...
    pub fn mocp_add_to_playlist(tab: &Tab) -> Result<()> {
        if !is_program_in_path(MOCP) {
            log_line!("mocp isn't installed");
            return Ok(());
        }
        Mocp::add_to_playlist(tab)
    }

    pub fn mocp_clear_playlist() -> Result<()> {
        if !is_program_in_path(MOCP) {
            log_line!("mocp isn't installed");
            return Ok(());
        }
        Mocp::clear()
    }

    /// Add a song or a folder to MOC playlist. Start it first...
    pub fn mocp_go_to_song(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        if !is_program_in_path(MOCP) {
            log_line!("mocp isn't installed");
            return Ok(());
        }
        Mocp::go_to_song(tab)?;

        status.update_second_pane_for_preview()
    }

    /// Toggle play/pause on MOC.
    /// Starts the server if needed, preventing the output to fill the screen.
    /// Then toggle play/pause
    pub fn mocp_toggle_pause(status: &mut Status) -> Result<()> {
        if !is_program_in_path(MOCP) {
            log_line!("mocp isn't installed");
            return Ok(());
        }
        Mocp::toggle_pause(status)
    }

    /// Skip to the next song in MOC
    pub fn mocp_next() -> Result<()> {
        if !is_program_in_path(MOCP) {
            log_line!("mocp isn't installed");
            return Ok(());
        }
        Mocp::next()
    }

    /// Go to the previous song in MOC
    pub fn mocp_previous() -> Result<()> {
        if !is_program_in_path(MOCP) {
            log_line!("mocp isn't installed");
            return Ok(());
        }
        Mocp::previous()
    }
}

/// Methods called when executing something with Enter key.
pub struct LeaveMode;

impl LeaveMode {
    /// Restore a file from the trash if possible.
    /// Parent folders are created if needed.
    pub fn trash(status: &mut Status) -> Result<()> {
        status.menu.trash.restore()?;
        status.current_tab_mut().reset_edit_mode();
        status.current_tab_mut().refresh_view()?;
        status.update_second_pane_for_preview()
    }

    /// Open the file with configured opener or enter the directory.
    pub fn open_file(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        if matches!(tab.display_mode, Display::Tree) {
            return EventAction::open_file(status);
        };
        if tab.path_content.is_empty() {
            return Ok(());
        }
        if tab.path_content.is_selected_dir()? {
            tab.go_to_selected_dir()
        } else {
            EventAction::open_file(status)
        }
    }

    /// Jump to the current mark.
    pub fn marks_jump(status: &mut Status) -> Result<()> {
        let marks = status.menu.marks.clone();
        let tab = status.current_tab_mut();
        if let Some((_, path)) = marks.selected() {
            tab.cd(path)?;
            tab.window.reset(tab.path_content.content.len());
            status.menu.input.reset();
        }
        status.update_second_pane_for_preview()
    }

    /// Update the selected mark with the current path.
    /// Doesn't change its char.
    /// If it doesn't fail, a new pair will be set with (oldchar, new path).
    pub fn marks_update(status: &mut Status) -> Result<()> {
        let marks = status.menu.marks.clone();
        let len = status.current_tab().path_content.content.len();
        if let Some((ch, _)) = marks.selected() {
            if let Some(path_str) = status.current_tab().path_content_str() {
                let p = path::PathBuf::from(path_str);
                status.menu.marks.new_mark(*ch, &p)?;
                log_line!("Saved mark {ch} -> {p}", p = p.display());
            }
            status.current_tab_mut().window.reset(len);
            status.menu.input.reset();
        }
        Ok(())
    }

    pub fn bulk(status: &mut Status) -> Result<()> {
        status.execute_bulk()?;
        status.menu.bulk = None;
        status.update_second_pane_for_preview()
    }

    pub fn shellmenu(status: &mut Status) -> Result<()> {
        status.menu.tui_applications.execute(status)
    }

    pub fn cli_info(status: &mut Status) -> Result<()> {
        let output = status.menu.cli_applications.execute()?;
        log_info!("output\n{output}");
        status.current_tab_mut().reset_edit_mode();
        status.current_tab_mut().set_display_mode(Display::Preview);
        let preview = Preview::cli_info(&output);
        status.current_tab_mut().window.reset(preview.len());
        status.current_tab_mut().preview = preview;
        Ok(())
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    pub fn chmod(status: &mut Status) -> Result<()> {
        status.chmod()
    }

    pub fn set_nvim_addr(status: &mut Status) -> Result<()> {
        status.nvim_server = status.menu.input.string();
        status.current_tab_mut().reset_edit_mode();
        Ok(())
    }

    /// Execute a jump to the selected flagged file.
    /// If the user selected a directory, we jump inside it.
    /// Otherwise, we jump to the parent and select the file.
    pub fn jump(status: &mut Status) -> Result<()> {
        let Some(jump_target) = status.menu.flagged.selected() else {
            return Ok(());
        };
        let jump_target = jump_target.to_owned();
        status.current_tab_mut().jump(jump_target)?;
        status.update_second_pane_for_preview()
    }

    /// Select the first file matching the typed regex in current dir.
    pub fn regex(status: &mut Status) -> Result<()> {
        status.select_from_regex()?;
        status.menu.input.reset();
        Ok(())
    }

    /// Execute a shell command typed by the user.
    /// pipes and redirections aren't supported
    /// but expansions are supported
    /// Returns `Ok(true)` if a refresh is required,
    /// `Ok(false)` if we should stay in the current mode (aka, a password is required)
    /// It won't return an `Err` if the command fail.
    pub fn shell(status: &mut Status) -> Result<bool> {
        status.parse_shell_command()
    }

    /// Execute a rename of the selected file.
    /// It uses the `fs::rename` function and has the same limitations.
    /// We only try to rename in the same directory, so it shouldn't be a problem.
    /// Filename is sanitized before processing.
    pub fn rename(status: &mut Status) -> Result<()> {
        let original_path = if let Display::Tree = status.current_tab().display_mode {
            status.current_tab().tree.selected_path()
        } else {
            status
                .current_tab()
                .path_content
                .selected()
                .context("rename: couldn't parse selected file")?
                .path
                .as_path()
        };
        if let Some(parent) = original_path.parent() {
            let new_path = parent.join(sanitize_filename::sanitize(status.menu.input.string()));
            log_info!(
                "renaming: original: {} - new: {}",
                original_path.display(),
                new_path.display()
            );
            log_line!(
                "renaming: original: {} - new: {}",
                original_path.display(),
                new_path.display()
            );

            fs::rename(original_path, new_path)?;
        }

        status.current_tab_mut().refresh_view()
    }

    /// Creates a new file with input string as name.
    /// Nothing is done if the file already exists.
    /// Filename is sanitized before processing.
    pub fn new_file(status: &mut Status) -> Result<()> {
        NodeCreation::Newfile.create(status)?;
        status.refresh_tabs()
    }

    /// Creates a new directory with input string as name.
    /// Nothing is done if the directory already exists.
    /// We use `fs::create_dir` internally so it will fail if the input string
    /// ie. the user can create `newdir` or `newdir/newfolder`.
    /// Directory name is sanitized before processing.
    pub fn new_dir(status: &mut Status) -> Result<()> {
        NodeCreation::Newdir.create(status)?;
        status.refresh_tabs()
    }

    /// Tries to execute the selected file with an executable which is read
    /// from the input string. It will fail silently if the executable can't
    /// be found.
    /// Optional parameters can be passed normally. ie. `"ls -lah"`
    pub fn exec(status: &mut Status) -> Result<()> {
        if status.current_tab().path_content.content.is_empty() {
            return Err(anyhow!("exec: empty directory"));
        }
        let exec_command = status.menu.input.string();
        let selected_file = &status
            .current_tab()
            .path_content
            .selected_path_string()
            .context("execute custom: no selected file")?;
        if let Ok(success) = execute_custom(exec_command, selected_file) {
            if success {
                status.menu.completion.reset();
                status.menu.input.reset();
            }
        }
        Ok(())
    }

    /// Executes a search in current folder, selecting the first file matching
    /// the current completion proposition.
    /// ie. If you typed `"jpg"` before, it will move to the first file
    /// whose filename contains `"jpg"`.
    /// The current order of files is used.
    pub fn search(status: &mut Status) -> Result<()> {
        let searched = &status.menu.input.string();
        status.menu.input.reset();
        if searched.is_empty() {
            status.current_tab_mut().searched = None;
            return Ok(());
        }
        status.current_tab_mut().searched = Some(searched.clone());
        match status.current_tab().display_mode {
            Display::Tree => {
                log_info!("searching in tree");
                status.current_tab_mut().tree.search_first_match(searched);
            }
            _ => {
                let next_index = status.current_tab().path_content.index;
                status.current_tab_mut().search_from(searched, next_index);
            }
        };
        status.update_second_pane_for_preview()
    }

    /// Move to the folder typed by the user.
    /// The first completion proposition is used, `~` expansion is done.
    /// If no result were found, no cd is done and we go back to normal mode
    /// silently.
    pub fn goto(status: &mut Status) -> Result<()> {
        if status.menu.completion.is_empty() {
            return Ok(());
        }
        let completed = status.menu.completion.current_proposition();
        let path = string_to_path(completed)?;
        status.menu.input.reset();
        status.current_tab_mut().cd(&path)?;
        let len = status.current_tab().path_content.content.len();
        status.current_tab_mut().window.reset(len);
        status.update_second_pane_for_preview()
    }

    /// Move to the selected shortcut.
    /// It may fail if the user has no permission to visit the path.
    pub fn shortcut(status: &mut Status) -> Result<()> {
        status.menu.input.reset();
        let path = status
            .menu
            .shortcut
            .selected()
            .context("exec shortcut: empty shortcuts")?
            .clone();
        status.current_tab_mut().cd(&path)?;
        status.current_tab_mut().refresh_view()?;
        status.update_second_pane_for_preview()
    }

    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    pub fn history(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        let (path, file) = tab
            .history
            .selected()
            .context("exec history: path unreachable")?
            .clone();
        tab.cd(&path)?;
        tab.history.drop_queue();
        let index = tab.path_content.select_file(&file);
        tab.scroll_to(index);
        log_info!("leave history {path:?} {file:?} {index}");
        status.update_second_pane_for_preview()
    }

    /// Execute the selected node if it's a file else enter the directory.
    pub fn tree(status: &mut Status) -> Result<()> {
        let path = status.current_tab().current_file()?.path;
        let is_dir = path.is_dir();
        if is_dir {
            status.current_tab_mut().cd(&path)?;
            status.current_tab_mut().make_tree(None)?;
            status.current_tab_mut().set_display_mode(Display::Tree);
            Ok(())
        } else {
            EventAction::open_file(status)
        }
    }

    /// Execute a password command (sudo or device passphrase).
    fn password(status: &mut Status) -> Result<()> {
        status.execute_password_command()
    }

    /// Compress the flagged files into an archive.
    /// Compression method is chosen by the user.
    /// The archive is created in the current directory and is named "archive.tar.??" or "archive.zip".
    /// Files which are above the CWD are filtered out since they can't be added to an archive.
    /// Archive creation depends on CWD so we ensure it's set to the selected tab.
    fn compress(status: &mut Status) -> Result<()> {
        let here = &status.current_tab().path_content.path;
        std::env::set_current_dir(here)?;
        let files_with_relative_paths: Vec<path::PathBuf> = status
            .menu
            .flagged
            .content
            .iter()
            .filter_map(|abs_path| pathdiff::diff_paths(abs_path, here))
            .filter(|f| !f.starts_with(".."))
            .collect();
        if files_with_relative_paths.is_empty() {
            return Ok(());
        }
        status
            .menu
            .compression
            .compress(files_with_relative_paths, here)
    }

    /// Execute the selected command.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    pub fn command(status: &mut Status, binds: &Bindings) -> Result<()> {
        let command_str = status.menu.completion.current_proposition();
        let Ok(command) = ActionMap::from_str(command_str) else {
            return Ok(());
        };
        command.matcher(status, binds)
    }

    /// A right click opens a file or a directory.
    pub fn right_click(status: &mut Status) -> Result<()> {
        match status.current_tab_mut().display_mode {
            Display::Normal => LeaveMode::open_file(status),
            Display::Tree => LeaveMode::tree(status),
            _ => Ok(()),
        }
    }

    /// Apply a filter to the displayed files.
    /// See `crate::filter` for more details.
    pub fn filter(status: &mut Status) -> Result<()> {
        let filter = FilterKind::from_input(&status.menu.input.string());
        status.current_tab_mut().settings.set_filter(filter);
        status.menu.input.reset();
        // ugly hack to please borrow checker :(
        status.tabs[status.index].path_content.reset_files(
            &status.tabs[status.index].settings,
            &status.tabs[status.index].users,
        )?;
        if let Display::Tree = status.current_tab().display_mode {
            status.current_tab_mut().make_tree(None)?;
        }
        let len = status.current_tab().path_content.content.len();
        status.current_tab_mut().window.reset(len);
        Ok(())
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    pub fn remote(status: &mut Status) -> Result<()> {
        let user_hostname_remotepath_string = status.menu.input.string();
        let strings: Vec<&str> = user_hostname_remotepath_string.split(' ').collect();
        status.menu.input.reset();

        if !is_program_in_path(SSHFS_EXECUTABLE) {
            log_info!("{SSHFS_EXECUTABLE} isn't in path");
            return Ok(());
        }

        if strings.len() != 3 {
            log_info!(
                "Wrong number of parameters for {SSHFS_EXECUTABLE}, expected 3, got {nb}",
                nb = strings.len()
            );
            return Ok(());
        };

        let (username, hostname, remote_path) = (strings[0], strings[1], strings[2]);
        let current_path: &str = &path_to_string(&status.current_tab().directory_of_selected()?);
        let first_arg = &format!("{username}@{hostname}:{remote_path}");
        let command_output = execute_and_capture_output_with_path(
            SSHFS_EXECUTABLE,
            current_path,
            &[first_arg, current_path],
        );
        log_info!("{SSHFS_EXECUTABLE} {strings:?} output {command_output:?}");
        log_line!("{SSHFS_EXECUTABLE} {strings:?} output {command_output:?}");
        Ok(())
    }
}
