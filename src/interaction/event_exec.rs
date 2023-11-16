use std::borrow::Borrow;
use std::fmt::Display;
use std::fs;
use std::path;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use log::info;

use super::ActionMap;
use crate::app::Status;
use crate::app::Tab;
use crate::completion::InputCompleted;
use crate::constant_strings_paths::{
    CONFIG_PATH, DEFAULT_DRAGNDROP, DIFF, GIO, MEDIAINFO, NITROGEN, SSHFS_EXECUTABLE,
};
use crate::display_mode::{ExtensionKind, Preview};
use crate::edit_mode::FilterKind;
use crate::edit_mode::Mocp;
use crate::edit_mode::RemovableDevices;
use crate::edit_mode::SelectableContent;
use crate::edit_mode::ShellCommandParser;
use crate::edit_mode::MOCP;
use crate::edit_mode::{lsblk_and_cryptsetup_installed, BlockDeviceAction};
use crate::edit_mode::{PasswordKind, PasswordUsage};
use crate::log::read_log;
use crate::log_line;
use crate::mode::DisplayMode;
use crate::mode::{EditMode, InputSimple, MarkAction, Navigate, NeedConfirmation};
use crate::opener::execute_and_capture_output_with_path;
use crate::opener::{
    execute_and_capture_output_without_check, execute_in_child,
    execute_in_child_without_output_with_path,
};
use crate::utils::path_to_string;
use crate::utils::{
    args_is_empty, is_program_in_path, is_sudo_command, open_in_current_neovim, string_to_path,
};

/// Links events from tuikit to custom actions.
/// It mutates `Status` or its children `Tab`.
pub struct EventAction {}

impl EventAction {
    /// Remove every flag on files in this directory and others.
    pub fn clear_flags(status: &mut Status) -> Result<()> {
        status.flagged.clear();
        Ok(())
    }

    /// Flag all files in the current directory.
    pub fn flag_all(status: &mut Status) -> Result<()> {
        status.tabs[status.index]
            .path_content
            .content
            .iter()
            .for_each(|file| {
                status.flagged.push(file.path.clone());
            });
        Ok(())
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn reverse_flags(status: &mut Status) -> Result<()> {
        status.tabs[status.index]
            .path_content
            .content
            .iter()
            .for_each(|file| status.flagged.toggle(&file.path));
        Ok(())
    }

    /// Toggle a single flag and move down one row.
    pub fn toggle_flag(status: &mut Status) -> Result<()> {
        let tab = status.selected_non_mut();

        if matches!(tab.edit_mode, EditMode::Nothing)
            && !matches!(tab.display_mode, DisplayMode::Preview)
        {
            let Ok(file) = tab.selected() else {
                return Ok(());
            };
            let path = file.path.clone();
            status.toggle_flag_on_path(&path);
            status.selected().normal_down_one_row();
        };
        Ok(())
    }

    /// Change to CHMOD mode allowing to edit permissions of a file.
    pub fn chmod(status: &mut Status) -> Result<()> {
        if status.selected().path_content.is_empty() {
            return Ok(());
        }
        status
            .selected()
            .set_edit_mode(EditMode::InputSimple(InputSimple::Chmod));
        if status.flagged.is_empty() {
            status
                .flagged
                .push(status.tabs[status.index].selected().unwrap().path.clone());
        };
        Ok(())
    }

    /// Enter JUMP mode, allowing to jump to any flagged file.
    /// Does nothing if no file is flagged.
    pub fn jump(status: &mut Status) -> Result<()> {
        if !status.flagged.is_empty() {
            status.flagged.index = 0;
            status
                .selected()
                .set_edit_mode(EditMode::Navigate(Navigate::Jump))
        }
        Ok(())
    }

    /// Enter Marks new mode, allowing to bind a char to a path.
    pub fn marks_new(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::Navigate(Navigate::Marks(MarkAction::New)));
        Ok(())
    }

    /// Enter Marks jump mode, allowing to jump to a marked file.
    pub fn marks_jump(status: &mut Status) -> Result<()> {
        if status.marks.is_empty() {
            return Ok(());
        }
        status
            .selected()
            .set_edit_mode(EditMode::Navigate(Navigate::Marks(MarkAction::Jump)));
        Ok(())
    }
    /// Creates a symlink of every flagged file to the current directory.
    pub fn symlink(status: &mut Status) -> Result<()> {
        for original_file in status.flagged.content.iter() {
            let filename = original_file
                .as_path()
                .file_name()
                .context("event symlink: File not found")?;
            let link = status
                .selected_non_mut()
                .directory_of_selected()?
                .join(filename);
            std::os::unix::fs::symlink(original_file, &link)?;
            log_line!(
                "Symlink {link} links to {original_file}",
                original_file = original_file.display(),
                link = link.display()
            );
        }
        status.clear_flags_and_reset_view()
    }

    /// Enter bulkrename mode, opening a random temp file where the user
    /// can edit the selected filenames.
    /// Once the temp file is saved, those file names are changed.
    pub fn bulk(status: &mut Status) -> Result<()> {
        status.init_bulk();
        status
            .selected()
            .set_edit_mode(EditMode::Navigate(Navigate::Bulk));
        Ok(())
    }

    /// Leave current mode to normal mode.
    /// Reset the inputs and completion, reset the window, exit the preview.
    pub fn reset_mode(tab: &mut Tab) -> Result<()> {
        if matches!(tab.display_mode, DisplayMode::Preview) {
            tab.set_display_mode(DisplayMode::Normal);
        }
        if tab.reset_edit_mode() {
            tab.refresh_view()
        } else {
            tab.refresh_params()
        }
    }
    /// Enter a copy paste mode.
    /// A confirmation is asked before copying all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn copy_paste(status: &mut Status) -> Result<()> {
        if status.flagged.is_empty() {
            return Ok(());
        }
        status
            .selected()
            .set_edit_mode(EditMode::NeedConfirmation(NeedConfirmation::Copy));
        Ok(())
    }

    /// Enter the 'move' mode.
    /// A confirmation is asked before moving all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn cut_paste(status: &mut Status) -> Result<()> {
        if status.flagged.is_empty() {
            return Ok(());
        }
        status
            .selected()
            .set_edit_mode(EditMode::NeedConfirmation(NeedConfirmation::Move));
        Ok(())
    }

    /// Enter the new dir mode.
    pub fn new_dir(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputSimple(InputSimple::Newdir));
        Ok(())
    }

    /// Enter the new file mode.
    pub fn new_file(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputSimple(InputSimple::Newfile));
        Ok(())
    }

    /// Enter the execute mode. Most commands must be executed to allow for
    /// a confirmation.
    pub fn exec(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputCompleted(InputCompleted::Exec));
        Ok(())
    }

    /// Preview the selected file.
    /// Every file can be previewed. See the `crate::enum::Preview` for
    /// more details on previewinga file.
    /// Does nothing if the directory is empty.
    pub fn preview(status: &mut Status) -> Result<()> {
        status.make_preview()
    }

    /// Enter the delete mode.
    /// A confirmation is then asked before deleting all the flagged files.
    /// If no file is flagged, flag the selected one before entering the mode.
    pub fn delete_file(status: &mut Status) -> Result<()> {
        if status.flagged.is_empty() {
            Self::toggle_flag(status)?;
        }
        status
            .selected()
            .set_edit_mode(EditMode::NeedConfirmation(NeedConfirmation::Delete));
        Ok(())
    }

    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn help(status: &mut Status) -> Result<()> {
        status.selected().set_display_mode(DisplayMode::Preview);
        status.selected().preview = Preview::help(&status.help);
        let len = status.selected_non_mut().preview.len();
        status.selected().window.reset(len);
        Ok(())
    }

    /// Display the last actions impacting the file tree
    pub fn log(tab: &mut Tab) -> Result<()> {
        let log = read_log()?;
        tab.set_display_mode(DisplayMode::Preview);
        tab.preview = Preview::log(log);
        tab.window.reset(tab.preview.len());
        tab.preview_go_bottom();
        Ok(())
    }

    /// Enter the search mode.
    /// Matching items are displayed as you type them.
    pub fn search(tab: &mut Tab) -> Result<()> {
        tab.searched = None;
        tab.set_edit_mode(EditMode::InputCompleted(InputCompleted::Search));
        Ok(())
    }

    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    pub fn regex_match(tab: &mut Tab) -> Result<()> {
        if !matches!(tab.edit_mode, EditMode::Nothing) {
            return Ok(());
        }
        match tab.display_mode {
            DisplayMode::Tree => (),
            _ => tab.set_edit_mode(EditMode::InputSimple(InputSimple::RegexMatch)),
        }
        Ok(())
    }

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn sort(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputSimple(InputSimple::Sort));
        Ok(())
    }

    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's usefull to reset the cursor before leaving the application.
    pub fn quit(tab: &mut Tab) -> Result<()> {
        tab.must_quit = true;
        Ok(())
    }
    /// Toggle the display of hidden files.
    pub fn toggle_hidden(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        tab.show_hidden = !tab.show_hidden;
        tab.path_content
            .reset_files(&tab.filter, tab.show_hidden, &tab.users)?;
        tab.window.reset(tab.path_content.content.len());
        if let DisplayMode::Tree = tab.display_mode {
            tab.make_tree(None)?
        }
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
        if status.flagged.is_empty() {
            status.open_selected_file()
        } else {
            status.open_flagged_files()
        }
    }

    /// Enter the rename mode.
    /// Keep a track of the current mode to ensure we rename the correct file.
    /// When we enter rename from a "tree" mode, we'll need to rename the selected file in the tree,
    /// not the selected file in the pathcontent.
    pub fn rename(tab: &mut Tab) -> Result<()> {
        tab.rename()
    }

    /// Enter the goto mode where an user can type a path to jump to.
    pub fn goto(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputCompleted(InputCompleted::Goto));
        tab.completion.reset();
        Ok(())
    }

    /// Open a new terminal in current directory.
    /// The shell is a fork of current process and will exit if the application
    /// is terminated first.
    pub fn shell(status: &mut Status) -> Result<()> {
        let tab = status.selected_non_mut();
        let path = tab.directory_of_selected()?;
        execute_in_child_without_output_with_path(&status.opener.terminal, path, None)?;
        Ok(())
    }

    pub fn shell_command(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputSimple(InputSimple::Shell));
        Ok(())
    }

    /// Enter the shell menu mode. You can pick a TUI application to be run
    pub fn shell_menu(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::Navigate(Navigate::ShellMenu));
        Ok(())
    }

    /// Enter the cli info mode. You can pick a Text application to be
    /// displayed/
    pub fn cli_info(status: &mut Status) -> Result<()> {
        status
            .selected()
            .set_edit_mode(EditMode::Navigate(Navigate::CliInfo));
        Ok(())
    }

    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    pub fn history(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::Navigate(Navigate::History));
        Ok(())
    }

    /// Enter the shortcut mode, allowing to visit predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    pub fn shortcut(tab: &mut Tab) -> Result<()> {
        std::env::set_current_dir(tab.directory_of_selected()?)?;
        tab.shortcut.update_git_root();
        tab.set_edit_mode(EditMode::Navigate(Navigate::Shortcut));
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
        if status.flagged.is_empty() {
            let Ok(fileinfo) = status.selected_non_mut().selected() else {
                return Ok(());
            };
            open_in_current_neovim(&fileinfo.path, &nvim_server);
        } else {
            let flagged = status.flagged.content.clone();
            for file_path in flagged.iter() {
                open_in_current_neovim(file_path, &nvim_server)
            }
        }

        Ok(())
    }

    pub fn set_nvim_server(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputSimple(InputSimple::SetNvimAddr));
        Ok(())
    }

    /// Enter the filter mode, where you can filter.
    /// See `crate::filter::Filter` for more details.
    pub fn filter(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputSimple(InputSimple::Filter));
        Ok(())
    }

    /// Move back in history to the last visited directory.
    pub fn back(tab: &mut Tab) -> Result<()> {
        tab.back()
    }

    /// Move to $HOME aka ~.
    pub fn home(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        let home_cow = shellexpand::tilde("~");
        let home: &str = home_cow.borrow();
        let path = std::fs::canonicalize(home)?;
        tab.set_pathcontent(&path)?;
        status.update_second_pane_for_preview()
    }

    pub fn go_root(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        let root_path = std::path::PathBuf::from("/");
        tab.set_pathcontent(&root_path)?;
        status.update_second_pane_for_preview()
    }

    pub fn go_start(status: &mut Status) -> Result<()> {
        let start_folder = status.start_folder.clone();
        status.selected().set_pathcontent(&start_folder)?;
        status.update_second_pane_for_preview()
    }

    /// Executes a `dragon-drop` command on the selected file.
    /// It obviously requires the `dragon-drop` command to be installed.
    pub fn drag_n_drop(status: &mut Status) -> Result<()> {
        if !is_program_in_path(DEFAULT_DRAGNDROP) {
            log_line!("{DEFAULT_DRAGNDROP} must be installed.");
            return Ok(());
        }
        let Ok(file) = status.selected_non_mut().selected() else {
            return Ok(());
        };
        let path_str = file
            .path
            .to_str()
            .context("event drag n drop: couldn't read path")?;

        execute_in_child(DEFAULT_DRAGNDROP, &[path_str])?;
        Ok(())
    }

    pub fn search_next(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        let Some(searched) = tab.searched.clone() else {
            return Ok(());
        };
        match tab.display_mode {
            DisplayMode::Tree => tab.tree.search_first_match(&searched),
            DisplayMode::Normal => tab.normal_search_next(&searched),
            DisplayMode::Preview => {
                return Ok(());
            }
        }
        status.set_second_pane_for_preview()?;
        Ok(())
    }

    /// Move up one row in modes allowing movement.
    /// Does nothing if the selected item is already the first in list.
    pub fn move_up(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.edit_mode {
            EditMode::Nothing => Self::move_display_up(status)?,
            EditMode::Navigate(Navigate::Jump) => status.flagged.prev(),
            EditMode::Navigate(Navigate::History) => tab.history.prev(),
            EditMode::Navigate(Navigate::Trash) => status.trash.prev(),
            EditMode::Navigate(Navigate::Shortcut) => tab.shortcut.prev(),
            EditMode::Navigate(Navigate::Marks(_)) => status.marks.prev(),
            EditMode::Navigate(Navigate::Compress) => status.compression.prev(),
            EditMode::Navigate(Navigate::Bulk) => status.bulk_prev(),
            EditMode::Navigate(Navigate::ShellMenu) => status.shell_menu.prev(),
            EditMode::Navigate(Navigate::CliInfo) => status.cli_info.prev(),
            EditMode::Navigate(Navigate::EncryptedDrive) => status.encrypted_devices.prev(),
            EditMode::InputCompleted(_) => tab.completion.prev(),
            _ => (),
        };
        status.update_second_pane_for_preview()
    }

    fn move_display_up(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.display_mode {
            DisplayMode::Normal => tab.normal_up_one_row(),
            DisplayMode::Preview => tab.preview_page_up(),
            DisplayMode::Tree => tab.tree_select_prev()?,
        }
        Ok(())
    }

    fn move_display_down(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.display_mode {
            DisplayMode::Normal => tab.normal_down_one_row(),
            DisplayMode::Preview => tab.preview_page_down(),
            DisplayMode::Tree => tab.tree_select_next()?,
        }
        Ok(())
    }
    /// Move down one row in modes allowing movements.
    /// Does nothing if the user is already at the bottom.
    pub fn move_down(status: &mut Status) -> Result<()> {
        match status.selected().edit_mode {
            EditMode::Nothing => Self::move_display_down(status)?,
            EditMode::Navigate(Navigate::Jump) => status.flagged.next(),
            EditMode::Navigate(Navigate::History) => status.selected().history.next(),
            EditMode::Navigate(Navigate::Trash) => status.trash.next(),
            EditMode::Navigate(Navigate::Shortcut) => status.selected().shortcut.next(),
            EditMode::Navigate(Navigate::Marks(_)) => status.marks.next(),
            EditMode::Navigate(Navigate::Compress) => status.compression.next(),
            EditMode::Navigate(Navigate::Bulk) => status.bulk_next(),
            EditMode::Navigate(Navigate::ShellMenu) => status.shell_menu.next(),
            EditMode::Navigate(Navigate::CliInfo) => status.cli_info.next(),
            EditMode::Navigate(Navigate::EncryptedDrive) => status.encrypted_devices.next(),
            EditMode::InputCompleted(_) => status.selected().completion.next(),
            _ => (),
        };
        status.update_second_pane_for_preview()
    }

    /// Move to parent in normal mode,
    /// move left one char in mode requiring text input.
    pub fn move_left(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.edit_mode {
            EditMode::InputSimple(_) | EditMode::InputCompleted(_) => {
                tab.input.cursor_left();
            }
            EditMode::Nothing => match tab.display_mode {
                DisplayMode::Normal => tab.move_to_parent()?,
                DisplayMode::Tree => tab.tree_select_parent()?,
                _ => (),
            },

            _ => (),
        }
        status.update_second_pane_for_preview()
    }

    /// Move to child if any or open a regular file in normal mode.
    /// Move the cursor one char to right in mode requiring text input.
    pub fn move_right(status: &mut Status) -> Result<()> {
        let tab: &mut Tab = status.selected();
        match tab.edit_mode {
            EditMode::InputSimple(_) | EditMode::InputCompleted(_) => {
                tab.input.cursor_right();
                Ok(())
            }
            EditMode::Nothing => match tab.display_mode {
                DisplayMode::Normal => LeaveMode::open_file(status),
                DisplayMode::Tree => {
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
    pub fn backspace(tab: &mut Tab) -> Result<()> {
        match tab.edit_mode {
            EditMode::InputSimple(_) | EditMode::InputCompleted(_) => {
                tab.input.delete_char_left();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Delete all chars to the right in mode allowing edition.
    pub fn delete(status: &mut Status) -> Result<()> {
        match status.selected().edit_mode {
            EditMode::InputSimple(_) | EditMode::InputCompleted(_) => {
                status.selected().input.delete_chars_right();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Move to leftmost char in mode allowing edition.
    pub fn key_home(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.edit_mode {
            EditMode::Nothing => {
                match tab.display_mode {
                    DisplayMode::Normal => tab.normal_go_top(),
                    DisplayMode::Preview => tab.preview_go_top(),
                    DisplayMode::Tree => tab.tree_go_to_root()?,
                };
            }
            _ => tab.input.cursor_start(),
        }
        status.update_second_pane_for_preview()
    }

    /// Move to the bottom in any mode.
    pub fn end(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.edit_mode {
            EditMode::Nothing => {
                match tab.display_mode {
                    DisplayMode::Normal => tab.normal_go_bottom(),
                    DisplayMode::Preview => tab.preview_go_bottom(),
                    DisplayMode::Tree => tab.tree_go_to_bottom_leaf()?,
                };
            }
            _ => tab.input.cursor_end(),
        }
        status.update_second_pane_for_preview()
    }

    /// Move up 10 lines in normal mode and preview.
    pub fn page_up(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.display_mode {
            DisplayMode::Normal => {
                tab.normal_page_up();
                status.update_second_pane_for_preview()?;
            }
            DisplayMode::Preview => tab.preview_page_up(),
            DisplayMode::Tree => {
                tab.tree_page_up();
                status.update_second_pane_for_preview()?;
            }
        };
        Ok(())
    }

    /// Move down 10 lines in normal & preview mode.
    pub fn page_down(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        match tab.display_mode {
            DisplayMode::Normal => {
                tab.normal_page_down();
                status.update_second_pane_for_preview()?;
            }
            DisplayMode::Preview => tab.preview_page_down(),
            DisplayMode::Tree => {
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
    pub fn enter(status: &mut Status) -> Result<()> {
        let mut must_refresh = true;
        let mut must_reset_mode = true;
        match status.selected_non_mut().edit_mode {
            EditMode::InputSimple(InputSimple::Rename) => LeaveMode::rename(status.selected())?,
            EditMode::InputSimple(InputSimple::Newfile) => LeaveMode::new_file(status.selected())?,
            EditMode::InputSimple(InputSimple::Newdir) => LeaveMode::new_dir(status.selected())?,
            EditMode::InputSimple(InputSimple::Chmod) => LeaveMode::chmod(status)?,
            EditMode::InputSimple(InputSimple::RegexMatch) => LeaveMode::regex(status)?,
            EditMode::InputSimple(InputSimple::SetNvimAddr) => LeaveMode::set_nvim_addr(status)?,
            EditMode::InputSimple(InputSimple::Shell) => {
                must_reset_mode = false;
                must_refresh = LeaveMode::shell(status)?;
            }
            EditMode::InputSimple(InputSimple::Filter) => {
                must_refresh = false;
                LeaveMode::filter(status.selected())?
            }
            EditMode::InputSimple(InputSimple::Password(kind, action, dest)) => {
                must_refresh = false;
                must_reset_mode = false;
                LeaveMode::password(status, kind, dest, action)?
            }
            EditMode::InputSimple(InputSimple::Remote) => LeaveMode::remote(status.selected())?,
            EditMode::Navigate(Navigate::Jump) => {
                must_refresh = false;
                LeaveMode::jump(status)?
            }
            EditMode::Navigate(Navigate::History) => {
                must_refresh = false;
                LeaveMode::history(status)?
            }
            EditMode::Navigate(Navigate::Shortcut) => LeaveMode::shortcut(status)?,
            EditMode::Navigate(Navigate::Trash) => LeaveMode::trash(status)?,
            EditMode::Navigate(Navigate::Bulk) => LeaveMode::bulk(status)?,
            EditMode::Navigate(Navigate::ShellMenu) => LeaveMode::shellmenu(status)?,
            EditMode::Navigate(Navigate::CliInfo) => {
                must_refresh = false;
                must_reset_mode = false;
                LeaveMode::cli_info(status)?;
            }
            EditMode::Navigate(Navigate::EncryptedDrive) => (),
            EditMode::Navigate(Navigate::Marks(MarkAction::New)) => {
                LeaveMode::marks_update(status)?
            }
            EditMode::Navigate(Navigate::Marks(MarkAction::Jump)) => LeaveMode::marks_jump(status)?,
            EditMode::Navigate(Navigate::Compress) => LeaveMode::compress(status)?,
            EditMode::Navigate(Navigate::RemovableDevices) => (),
            EditMode::InputCompleted(InputCompleted::Exec) => LeaveMode::exec(status.selected())?,
            EditMode::InputCompleted(InputCompleted::Search) => {
                must_refresh = false;
                LeaveMode::search(status)?
            }
            EditMode::InputCompleted(InputCompleted::Goto) => LeaveMode::goto(status)?,
            EditMode::InputCompleted(InputCompleted::Command) => LeaveMode::command(status)?,
            EditMode::NeedConfirmation(_)
            | EditMode::InputCompleted(InputCompleted::Nothing)
            | EditMode::InputSimple(InputSimple::Sort) => (),
            EditMode::Nothing => match status.selected_non_mut().display_mode {
                DisplayMode::Normal => {
                    LeaveMode::open_file(status)?;
                    must_reset_mode = false;
                }
                DisplayMode::Tree => LeaveMode::tree(status)?,
                _ => (),
            },
        };

        status.selected().input.reset();
        if must_reset_mode {
            status.selected().reset_edit_mode();
        }
        if must_refresh {
            status.refresh_status()?;
        }
        Ok(())
    }

    /// Change tab in normal mode with dual pane displayed,
    /// insert a completion in modes allowing completion.
    pub fn tab(status: &mut Status) -> Result<()> {
        match status.selected().edit_mode {
            EditMode::InputCompleted(_) => {
                let tab = status.selected();
                tab.input.replace(tab.completion.current_proposition())
            }
            EditMode::Nothing => status.next(),
            _ => (),
        };
        Ok(())
    }

    /// Change tab in normal mode.
    pub fn backtab(status: &mut Status) -> Result<()> {
        if matches!(status.selected_non_mut().edit_mode, EditMode::Nothing) {
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
    pub fn fuzzyfind_help(status: &mut Status) -> Result<()> {
        status.skim_find_keybinding()
    }

    /// Copy the filename of the selected file in normal mode.
    pub fn copy_filename(tab: &mut Tab) -> Result<()> {
        if let DisplayMode::Normal | DisplayMode::Tree = tab.display_mode {
            return tab.filename_to_clipboard();
        }
        Ok(())
    }

    /// Copy the filepath of the selected file in normal mode.
    pub fn copy_filepath(tab: &mut Tab) -> Result<()> {
        if let DisplayMode::Normal | DisplayMode::Tree = tab.display_mode {
            return tab.filepath_to_clipboard();
        }
        Ok(())
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn refreshview(status: &mut Status) -> Result<()> {
        status.encrypted_devices.update()?;
        status.refresh_status()?;
        status.update_second_pane_for_preview()
    }

    /// Refresh the view if files were modified in current directory.
    pub fn refresh_if_needed(tab: &mut Tab) -> Result<()> {
        tab.refresh_if_needed()
    }

    /// Display mediainfo details of an image
    pub fn mediainfo(tab: &mut Tab) -> Result<()> {
        if !is_program_in_path(MEDIAINFO) {
            log_line!("{} isn't installed", MEDIAINFO);
            return Ok(());
        }
        if let DisplayMode::Normal | DisplayMode::Tree = tab.display_mode {
            let Ok(file_info) = tab.selected() else {
                return Ok(());
            };
            info!("selected {:?}", file_info);
            tab.preview = Preview::mediainfo(&file_info.path)?;
            tab.window.reset(tab.preview.len());
            tab.set_display_mode(DisplayMode::Preview);
        }
        Ok(())
    }

    /// Display a diff between the first 2 flagged files or dir.
    pub fn diff(status: &mut Status) -> Result<()> {
        if !is_program_in_path(DIFF) {
            log_line!("{DIFF} isn't installed");
            return Ok(());
        }
        if status.flagged.len() < 2 {
            return Ok(());
        };
        if let DisplayMode::Normal | DisplayMode::Tree = status.selected_non_mut().display_mode {
            let first_path = &status.flagged.content[0]
                .to_str()
                .context("Couldn't parse filename")?;
            let second_path = &status.flagged.content[1]
                .to_str()
                .context("Couldn't parse filename")?;
            status.selected().preview = Preview::diff(first_path, second_path)?;
            let tab = status.selected();
            tab.window.reset(tab.preview.len());
            tab.set_display_mode(DisplayMode::Preview);
        }
        Ok(())
    }

    /// Toggle between a full display (aka ls -lah) or a simple mode (only the
    /// filenames).
    pub fn toggle_display_full(status: &mut Status) -> Result<()> {
        status.display_full = !status.display_full;
        Ok(())
    }

    /// Toggle between dualpane and single pane. Does nothing if the width
    /// is too low to display both panes.
    pub fn toggle_dualpane(status: &mut Status) -> Result<()> {
        status.dual_pane = !status.dual_pane;
        status.select_tab(0)?;
        Ok(())
    }

    /// Move flagged files to the trash directory.
    /// If no file is flagged, flag the selected file.
    /// More information in the trash crate itself.
    /// If the file is mounted on the $topdir of the trash (aka the $HOME mount point),
    /// it is moved there.
    /// Else, nothing is done.
    pub fn trash_move_file(status: &mut Status) -> Result<()> {
        if status.flagged.is_empty() {
            Self::toggle_flag(status)?;
        }

        status.trash.update()?;
        for flagged in status.flagged.content.iter() {
            status.trash.trash(flagged)?;
        }
        status.flagged.clear();
        status.selected().refresh_view()?;
        Ok(())
    }
    /// Ask the user if he wants to empty the trash.
    /// It requires a confimation before doing anything
    pub fn trash_empty(status: &mut Status) -> Result<()> {
        status.trash.update()?;
        status
            .selected()
            .set_edit_mode(EditMode::NeedConfirmation(NeedConfirmation::EmptyTrash));
        Ok(())
    }

    /// Open the trash.
    /// Displays a navigable content of the trash.
    /// Each item can be restored or deleted.
    /// Each opening refresh the trash content.
    pub fn trash_open(status: &mut Status) -> Result<()> {
        status.trash.update()?;
        status
            .selected()
            .set_edit_mode(EditMode::Navigate(Navigate::Trash));
        Ok(())
    }

    /// Creates a tree in every mode but "Tree".
    /// It tree mode it will exit this view.
    pub fn tree(status: &mut Status) -> Result<()> {
        status.tree()
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

    /// Enter the encrypted device menu, allowing the user to mount/umount
    /// a luks encrypted device.
    pub fn encrypted_drive(status: &mut Status) -> Result<()> {
        if !lsblk_and_cryptsetup_installed() {
            log_line!("lsblk and cryptsetup must be installed.");
            return Ok(());
        }
        if status.encrypted_devices.is_empty() {
            status.encrypted_devices.update()?;
        }
        status
            .selected()
            .set_edit_mode(EditMode::Navigate(Navigate::EncryptedDrive));
        Ok(())
    }

    pub fn removable_devices(status: &mut Status) -> Result<()> {
        if !is_program_in_path(GIO) {
            log_line!("gio must be installed.");
            return Ok(());
        }
        status.removable_devices = RemovableDevices::from_gio();
        status
            .selected()
            .set_edit_mode(EditMode::Navigate(Navigate::RemovableDevices));
        Ok(())
    }

    /// Open the config file.
    pub fn open_config(status: &mut Status) -> Result<()> {
        match status.opener.open(&path::PathBuf::from(
            shellexpand::tilde(CONFIG_PATH).to_string(),
        )) {
            Ok(_) => (),
            Err(e) => info!("Error opening {:?}: the config file {}", CONFIG_PATH, e),
        }
        Ok(())
    }

    /// Enter compression mode
    pub fn compress(status: &mut Status) -> Result<()> {
        status
            .selected()
            .set_edit_mode(EditMode::Navigate(Navigate::Compress));
        Ok(())
    }

    /// Enter command mode in which you can type any valid command.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    pub fn command(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputCompleted(InputCompleted::Command));
        tab.completion.reset();
        Ok(())
    }

    /// Toggle the second pane between preview & normal mode (files).
    pub fn toggle_preview_second(status: &mut Status) -> Result<()> {
        status.preview_second = !status.preview_second;
        if status.preview_second {
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
        let _ = execute_in_child(NITROGEN, &["--set-zoom-fill", "--save", &path_str]);
        Ok(())
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
        let tab = status.selected();
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

    /// Execute a custom event on the selected file
    pub fn custom(status: &mut Status, string: &String) -> Result<()> {
        info!("custom {string}");
        let parser = ShellCommandParser::new(string);
        let mut args = parser.compute(status)?;
        let command = args.remove(0);
        let args: Vec<&str> = args.iter().map(|s| &**s).collect();
        let output = execute_and_capture_output_without_check(command, &args)?;
        info!("output {output}");
        Ok(())
    }

    pub fn remote_mount(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(EditMode::InputSimple(InputSimple::Remote));
        Ok(())
    }
}

enum NodeCreation {
    Newfile,
    Newdir,
}

impl Display for NodeCreation {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Newfile => write!(f, "file"),
            Self::Newdir => write!(f, "directory"),
        }
    }
}

impl NodeCreation {
    fn create(&self, tab: &mut Tab) -> Result<()> {
        let root_path = match tab.display_mode {
            DisplayMode::Tree => tab
                .tree
                .directory_of_selected()
                .context("no parent")?
                .to_owned(),
            _ => tab.path_content.path.clone(),
        };
        log::info!("root_path: {root_path:?}");
        let path = root_path.join(sanitize_filename::sanitize(tab.input.string()));
        if path.exists() {
            log_line!("{self} {path} already exists", path = path.display());
        } else {
            match self {
                Self::Newdir => {
                    fs::create_dir_all(&path)?;
                }
                Self::Newfile => {
                    fs::File::create(&path)?;
                }
            }
            log_line!("Created new {self}: {path}", path = path.display());
        }
        tab.refresh_view()
    }
}

/// Methods called when executing something with Enter key.
pub struct LeaveMode;

impl LeaveMode {
    /// Restore a file from the trash if possible.
    /// Parent folders are created if needed.
    pub fn trash(status: &mut Status) -> Result<()> {
        status.trash.restore()?;
        status.selected().reset_edit_mode();
        status.selected().refresh_view()?;
        status.update_second_pane_for_preview()
    }

    /// Open the file with configured opener or enter the directory.
    pub fn open_file(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        if matches!(tab.display_mode, DisplayMode::Tree) {
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
        let marks = status.marks.clone();
        let tab = status.selected();
        if let Some((_, path)) = marks.selected() {
            tab.set_pathcontent(path)?;
            tab.window.reset(tab.path_content.content.len());
            tab.input.reset();
        }
        status.update_second_pane_for_preview()
    }

    /// Update the selected mark with the current path.
    /// Doesn't change its char.
    /// If it doesn't fail, a new pair will be set with (oldchar, new path).
    pub fn marks_update(status: &mut Status) -> Result<()> {
        let marks = status.marks.clone();
        let len = status.selected_non_mut().path_content.content.len();
        if let Some((ch, _)) = marks.selected() {
            if let Some(path_str) = status.selected_non_mut().path_content_str() {
                let p = path::PathBuf::from(path_str);
                status.marks.new_mark(*ch, &p)?;
                log_line!("Saved mark {ch} -> {p}", p = p.display());
            }
            status.selected().window.reset(len);
            status.selected().input.reset();
        }
        Ok(())
    }

    pub fn bulk(status: &mut Status) -> Result<()> {
        status.execute_bulk()?;
        status.bulk = None;
        status.update_second_pane_for_preview()
    }

    pub fn shellmenu(status: &mut Status) -> Result<()> {
        status.shell_menu.execute(status)
    }

    pub fn cli_info(status: &mut Status) -> Result<()> {
        let output = status.cli_info.execute()?;
        info!("output\n{output}");
        status.selected().set_display_mode(DisplayMode::Preview);
        let preview = Preview::cli_info(&output);
        status.selected().window.reset(preview.len());
        status.selected().preview = preview;
        Ok(())
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    pub fn chmod(status: &mut Status) -> Result<()> {
        if status.selected().input.is_empty() || status.flagged.is_empty() {
            return Ok(());
        }
        let input_permission = &status.selected().input.string();
        let permissions: u32 = u32::from_str_radix(input_permission, 8).unwrap_or(0_u32);
        if permissions <= Status::MAX_PERMISSIONS {
            for path in status.flagged.content.iter() {
                Status::set_permissions(path, permissions)?
            }
            status.flagged.clear();
            log_line!("Changed permissions to {input_permission}");
        }
        status.selected().refresh_view()?;
        status.reset_tabs_view()
    }

    pub fn set_nvim_addr(status: &mut Status) -> Result<()> {
        status.nvim_server = status.selected_non_mut().input.string();
        status.selected().reset_edit_mode();
        Ok(())
    }

    /// Execute a jump to the selected flagged file.
    /// If the user selected a directory, we jump inside it.
    /// Otherwise, we jump to the parent and select the file.
    pub fn jump(status: &mut Status) -> Result<()> {
        let Some(jump_target) = status.flagged.selected() else {
            return Ok(());
        };
        let jump_target = jump_target.to_owned();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent,
            None => &jump_target,
        };
        status.selected().set_pathcontent(target_dir)?;
        let index = status.selected().path_content.select_file(&jump_target);
        status.selected().scroll_to(index);
        status.update_second_pane_for_preview()
    }

    /// Select the first file matching the typed regex in current dir.
    pub fn regex(status: &mut Status) -> Result<(), regex::Error> {
        status.select_from_regex()?;
        status.selected().input.reset();
        Ok(())
    }

    /// Execute a shell command typed by the user.
    /// pipes and redirections aren't supported
    /// but expansions are supported
    /// Returns `Ok(true)` if a refresh is required,
    /// `Ok(false)` if we should stay in the current mode (aka, a password is required)
    /// It won't return an `Err` if the command fail.
    pub fn shell(status: &mut Status) -> Result<bool> {
        let shell_command = status.selected_non_mut().input.string();
        let mut args = ShellCommandParser::new(&shell_command).compute(status)?;
        info!("command {shell_command} args: {args:?}");
        if args_is_empty(&args) {
            status.selected().set_edit_mode(EditMode::Nothing);
            return Ok(true);
        }
        let executable = args.remove(0);
        if is_sudo_command(&executable) {
            status.sudo_command = Some(shell_command);
            status.ask_password(PasswordKind::SUDO, None, PasswordUsage::SUDOCOMMAND)?;
            Ok(false)
        } else {
            if !is_program_in_path(&executable) {
                return Ok(true);
            }
            let current_directory = status
                .selected_non_mut()
                .directory_of_selected()?
                .to_owned();
            let params: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            execute_in_child_without_output_with_path(
                executable,
                current_directory,
                Some(&params),
            )?;
            status.selected().set_edit_mode(EditMode::Nothing);
            Ok(true)
        }
    }

    /// Execute a rename of the selected file.
    /// It uses the `fs::rename` function and has the same limitations.
    /// We only try to rename in the same directory, so it shouldn't be a problem.
    /// Filename is sanitized before processing.
    pub fn rename(tab: &mut Tab) -> Result<()> {
        let original_path = if let DisplayMode::Tree = tab.display_mode {
            tab.tree.selected_path()
        } else {
            tab.path_content
                .selected()
                .context("rename: couldn't parse selected file")?
                .path
                .as_path()
        };
        if let Some(parent) = original_path.parent() {
            let new_path = parent.join(sanitize_filename::sanitize(tab.input.string()));
            info!(
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

        tab.refresh_view()
    }

    /// Creates a new file with input string as name.
    /// Nothing is done if the file already exists.
    /// Filename is sanitized before processing.
    pub fn new_file(tab: &mut Tab) -> Result<()> {
        NodeCreation::Newfile.create(tab)
    }

    /// Creates a new directory with input string as name.
    /// Nothing is done if the directory already exists.
    /// We use `fs::create_dir` internally so it will fail if the input string
    /// ie. the user can create `newdir` or `newdir/newfolder`.
    /// Directory name is sanitized before processing.
    pub fn new_dir(tab: &mut Tab) -> Result<()> {
        NodeCreation::Newdir.create(tab)
    }

    /// Tries to execute the selected file with an executable which is read
    /// from the input string. It will fail silently if the executable can't
    /// be found.
    /// Optional parameters can be passed normally. ie. `"ls -lah"`
    pub fn exec(tab: &mut Tab) -> Result<()> {
        if tab.path_content.content.is_empty() {
            return Err(anyhow!("exec: empty directory"));
        }
        let exec_command = tab.input.string();
        if let Ok(success) = tab.execute_custom(exec_command) {
            if success {
                tab.completion.reset();
                tab.input.reset();
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
        let tab = status.selected();
        let searched = &tab.input.string();
        tab.input.reset();
        if searched.is_empty() {
            tab.searched = None;
            return Ok(());
        }
        tab.searched = Some(searched.clone());
        match tab.display_mode {
            DisplayMode::Tree => {
                log::info!("searching in tree");
                tab.tree.search_first_match(searched);
            }
            _ => {
                let next_index = tab.path_content.index;
                tab.search_from(searched, next_index);
            }
        };
        status.update_second_pane_for_preview()
    }

    /// Move to the folder typed by the user.
    /// The first completion proposition is used, `~` expansion is done.
    /// If no result were found, no cd is done and we go back to normal mode
    /// silently.
    pub fn goto(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        if tab.completion.is_empty() {
            return Ok(());
        }
        let completed = tab.completion.current_proposition();
        let path = string_to_path(completed)?;
        tab.input.reset();
        tab.set_pathcontent(&path)?;
        tab.window.reset(tab.path_content.content.len());
        status.update_second_pane_for_preview()
    }

    /// Move to the selected shortcut.
    /// It may fail if the user has no permission to visit the path.
    pub fn shortcut(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        tab.input.reset();
        let path = tab
            .shortcut
            .selected()
            .context("exec shortcut: empty shortcuts")?
            .clone();
        tab.set_pathcontent(&path)?;
        tab.refresh_view()?;
        status.update_second_pane_for_preview()
    }

    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    pub fn history(status: &mut Status) -> Result<()> {
        let tab = status.selected();
        let (path, file) = tab
            .history
            .selected()
            .context("exec history: path unreachable")?
            .clone();
        tab.set_pathcontent(&path)?;
        tab.history.drop_queue();
        let index = tab.path_content.select_file(&file);
        tab.scroll_to(index);
        log::info!("leave history {path:?} {file:?} {index}");
        status.update_second_pane_for_preview()
    }

    /// Execute the selected node if it's a file else enter the directory.
    pub fn tree(status: &mut Status) -> Result<()> {
        let path = status.selected_non_mut().selected()?.path;
        let is_dir = path.is_dir();
        if is_dir {
            status.selected().set_pathcontent(&path)?;
            status.selected().make_tree(None)?;
            status.selected().set_display_mode(DisplayMode::Tree);
            Ok(())
        } else {
            EventAction::open_file(status)
        }
    }

    /// Store a password of some kind (sudo or device passphrase).
    fn password(
        status: &mut Status,
        password_kind: PasswordKind,
        dest: PasswordUsage,
        action: Option<BlockDeviceAction>,
    ) -> Result<()> {
        let password = status.selected_non_mut().input.string();
        match password_kind {
            PasswordKind::SUDO => status.password_holder.set_sudo(password),
            PasswordKind::CRYPTSETUP => status.password_holder.set_cryptsetup(password),
        }
        status.selected().reset_edit_mode();
        status.dispatch_password(dest, action)
    }

    /// Compress the flagged files into an archive.
    /// Compression method is chosen by the user.
    /// The archive is created in the current directory and is named "archive.tar.??" or "archive.zip".
    /// Files which are above the CWD are filtered out since they can't be added to an archive.
    /// Archive creation depends on CWD so we ensure it's set to the selected tab.
    fn compress(status: &mut Status) -> Result<()> {
        let here = &status.selected_non_mut().path_content.path;
        std::env::set_current_dir(here)?;
        let files_with_relative_paths: Vec<path::PathBuf> = status
            .flagged
            .content
            .iter()
            .filter_map(|abs_path| pathdiff::diff_paths(abs_path, here))
            .filter(|f| !f.starts_with(".."))
            .collect();
        if files_with_relative_paths.is_empty() {
            return Ok(());
        }
        status.compression.compress(files_with_relative_paths, here)
    }

    /// Execute the selected command.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    pub fn command(status: &mut Status) -> Result<()> {
        let command_str = status.selected_non_mut().completion.current_proposition();
        let Ok(command) = ActionMap::from_str(command_str) else {
            return Ok(());
        };
        command.matcher(status)
    }

    /// A right click opens a file or a directory.
    pub fn right_click(status: &mut Status) -> Result<()> {
        match status.selected().display_mode {
            DisplayMode::Normal => LeaveMode::open_file(status),
            DisplayMode::Tree => LeaveMode::tree(status),
            _ => Ok(()),
        }
    }

    /// Apply a filter to the displayed files.
    /// See `crate::filter` for more details.
    pub fn filter(tab: &mut Tab) -> Result<()> {
        let filter = FilterKind::from_input(&tab.input.string());
        tab.set_filter(filter);
        tab.input.reset();
        tab.path_content
            .reset_files(&tab.filter, tab.show_hidden, &tab.users)?;
        if let DisplayMode::Tree = tab.display_mode {
            tab.make_tree(None)?;
        }
        tab.window.reset(tab.path_content.content.len());
        Ok(())
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    pub fn remote(tab: &mut Tab) -> Result<()> {
        let user_hostname_remotepath_string = tab.input.string();
        let strings: Vec<&str> = user_hostname_remotepath_string.split(' ').collect();
        tab.input.reset();

        if !is_program_in_path(SSHFS_EXECUTABLE) {
            info!("{SSHFS_EXECUTABLE} isn't in path");
            return Ok(());
        }

        if strings.len() != 3 {
            info!(
                "Wrong number of parameters for {SSHFS_EXECUTABLE}, expected 3, got {nb}",
                nb = strings.len()
            );
            return Ok(());
        };

        let (username, hostname, remote_path) = (strings[0], strings[1], strings[2]);
        let current_path: &str = &path_to_string(&tab.directory_of_selected()?);
        let first_arg = &format!("{username}@{hostname}:{remote_path}");
        let command_output = execute_and_capture_output_with_path(
            SSHFS_EXECUTABLE,
            current_path,
            &[first_arg, current_path],
        );
        info!("{SSHFS_EXECUTABLE} {strings:?} output {command_output:?}");
        log_line!("{SSHFS_EXECUTABLE} {strings:?} output {command_output:?}");
        Ok(())
    }
}
