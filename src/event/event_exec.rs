use std::borrow::Borrow;
use std::path;

use anyhow::{Context, Result};

use crate::app::Status;
use crate::app::Tab;
use crate::common::LAZYGIT;
use crate::common::NCDU;
use crate::common::{is_program_in_path, open_in_current_neovim};
use crate::common::{CONFIG_PATH, GIO};
use crate::config::Bindings;
use crate::config::START_FOLDER;
use crate::io::execute_without_output_with_path;
use crate::io::read_log;
use crate::log_info;
use crate::log_line;
use crate::modes::help_string;
use crate::modes::lsblk_and_cryptsetup_installed;
use crate::modes::Display;
use crate::modes::Edit;
use crate::modes::InputCompleted;
use crate::modes::InputSimple;
use crate::modes::LeaveMode;
use crate::modes::MarkAction;
use crate::modes::Mocp;
use crate::modes::Navigate;
use crate::modes::NeedConfirmation;
use crate::modes::Preview;
use crate::modes::RemovableDevices;
use crate::modes::SelectableContent;
use crate::modes::TuiApplications;
use crate::modes::MOCP;

/// Links events from tuikit to custom actions.
/// It mutates `Status` or its children `Tab`.
pub struct EventAction {}

impl EventAction {
    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's usefull to reset the cursor before leaving the application.
    pub fn quit(status: &mut Status) -> Result<()> {
        status.internal_settings.must_quit = true;
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
    pub fn reset_mode(status: &mut Status) -> Result<()> {
        if matches!(status.current_tab().display_mode, Display::Preview) {
            status.tabs[status.index].set_display_mode(Display::Directory);
        }
        status.menu.input.reset();
        status.menu.completion.reset();
        if status.reset_edit_mode()? {
            status.tabs[status.index].refresh_view()
        } else {
            status.tabs[status.index].refresh_params()
        }
    }

    /// Toggle between a full display (aka ls -lah) or a simple mode (only the
    /// filenames).
    pub fn toggle_display_full(status: &mut Status) -> Result<()> {
        status.display_settings.toggle_metadata();
        Ok(())
    }

    /// Toggle between dualpane and single pane. Does nothing if the width
    /// is too low to display both panes.
    pub fn toggle_dualpane(status: &mut Status) -> Result<()> {
        status.display_settings.toggle_dual();
        status.select_left();
        Ok(())
    }

    /// Toggle the second pane between preview & normal mode (files).
    pub fn toggle_preview_second(status: &mut Status) -> Result<()> {
        status.display_settings.toggle_preview();
        if status.display_settings.preview() {
            status.set_second_pane_for_preview()?;
        } else {
            status.set_edit_mode(1, Edit::Nothing)?;
            status.tabs[1].display_mode = Display::Directory;
            status.tabs[1].refresh_view()?;
        }
        Ok(())
    }

    /// Creates a tree in every mode but "Tree".
    /// In display_mode tree it will exit this view.
    pub fn tree(status: &mut Status) -> Result<()> {
        status.current_tab_mut().toggle_tree_mode()?;
        status.refresh_view()
    }

    /// Fold the current node of the tree.
    /// Has no effect on "file" nodes.
    pub fn tree_fold(tab: &mut Tab) -> Result<()> {
        tab.tree.toggle_fold(&tab.users);
        Ok(())
    }

    /// Unfold every child node in the tree.
    /// Recursively explore the tree and unfold every node.
    /// Reset the display.
    pub fn tree_unfold_all(tab: &mut Tab) -> Result<()> {
        tab.tree.unfold_all(&tab.users);
        Ok(())
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node.
    /// Reset the display.
    pub fn tree_fold_all(tab: &mut Tab) -> Result<()> {
        tab.tree.fold_all(&tab.users);
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
        let sel_path = selected.path;
        if sel_path == status.current_tab().directory.path {
            return Ok(());
        }
        if let Some(parent) = status.current_tab().directory.path.parent() {
            if sel_path == std::rc::Rc::from(parent) {
                return Ok(());
            }
        }
        let old_name = &selected.filename;
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Rename))?;
        status.menu.input.replace(old_name);
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
        status.set_edit_mode(status.index, Edit::NeedConfirmation(copy_or_move))
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

    /// Enter the delete mode.
    /// A confirmation is then asked before deleting all the flagged files.
    /// If no file is flagged, flag the selected one before entering the mode.
    pub fn delete_file(status: &mut Status) -> Result<()> {
        if status.menu.flagged.is_empty() {
            Self::toggle_flag(status)?;
        }
        status.set_edit_mode(
            status.index,
            Edit::NeedConfirmation(NeedConfirmation::Delete),
        )
    }

    /// Change to CHMOD mode allowing to edit permissions of a file.
    pub fn chmod(status: &mut Status) -> Result<()> {
        status.set_mode_chmod()
    }

    /// Enter the new dir mode.
    pub fn new_dir(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Newdir))
    }

    /// Enter the new file mode.
    pub fn new_file(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Newfile))
    }

    fn enter_file(status: &mut Status) -> Result<()> {
        match status.current_tab_mut().display_mode {
            Display::Directory => Self::normal_enter_file(status),
            Display::Tree => Self::tree_enter_file(status),
            _ => Ok(()),
        }
    }

    /// Open the file with configured opener or enter the directory.
    fn normal_enter_file(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        if matches!(tab.display_mode, Display::Tree) {
            return EventAction::open_file(status);
        };
        if tab.directory.is_empty() {
            return Ok(());
        }
        if tab.directory.is_selected_dir()? {
            tab.go_to_selected_dir()
        } else {
            EventAction::open_file(status)
        }
    }

    /// Execute the selected node if it's a file else enter the directory.
    fn tree_enter_file(status: &mut Status) -> Result<()> {
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

    /// Enter the execute mode. Most commands must be executed to allow for
    /// a confirmation.
    pub fn exec(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Exec))
    }

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn sort(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Sort))
    }

    /// Enter the filter mode, where you can filter.
    /// See `crate::modes::Filter` for more details.
    pub fn filter(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Filter))
    }

    /// Enter JUMP mode, allowing to jump to any flagged file.
    /// Does nothing if no file is flagged.
    pub fn jump(status: &mut Status) -> Result<()> {
        if !status.menu.flagged.is_empty() {
            status.menu.flagged.index = 0;
            status.set_edit_mode(status.index, Edit::Navigate(Navigate::Jump))?;
        }
        Ok(())
    }

    /// Enter bulkrename mode, opening a random temp file where the user
    /// can edit the selected filenames.
    /// Once the temp file is saved, those file names are changed.
    pub fn bulk(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::BulkMenu))
    }

    /// Enter the search mode.
    /// Matching items are displayed as you type them.
    pub fn search(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        tab.searched = None;
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Search))
    }

    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    pub fn regex_match(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::RegexMatch))
    }

    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn help(status: &mut Status, binds: &Bindings) -> Result<()> {
        let help = help_string(binds, &status.internal_settings.opener);
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

    /// Enter the goto mode where an user can type a path to jump to.
    pub fn goto(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Goto))?;
        status.menu.completion.reset();
        Ok(())
    }

    /// Open a new terminal in current directory.
    /// The shell is a fork of current process and will exit if the application
    /// is terminated first.
    pub fn shell(status: &mut Status) -> Result<()> {
        let tab = status.current_tab();
        let path = tab.directory_of_selected()?;
        execute_without_output_with_path(&status.internal_settings.opener.terminal, path, None)?;
        Ok(())
    }

    /// Enter the shell input command mode. The user can type a command which
    /// will be parsed and run.
    pub fn shell_command(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Shell))
    }

    /// Enter the shell menu mode. You can pick a TUI application to be run
    pub fn tui_menu(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::TuiApplication))
    }

    /// Enter the cli info mode. You can pick a Text application to be
    /// displayed/
    pub fn cli_menu(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::CliApplication))
    }

    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    pub fn history(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::History))
    }

    /// Enter Marks new mode, allowing to bind a char to a path.
    pub fn marks_new(status: &mut Status) -> Result<()> {
        status.set_edit_mode(
            status.index,
            Edit::Navigate(Navigate::Marks(MarkAction::New)),
        )
    }

    /// Enter Marks jump mode, allowing to jump to a marked file.
    pub fn marks_jump(status: &mut Status) -> Result<()> {
        if status.menu.marks.is_empty() {
            return Ok(());
        }
        status.set_edit_mode(
            status.index,
            Edit::Navigate(Navigate::Marks(MarkAction::Jump)),
        )
    }

    /// Enter the shortcut mode, allowing to visit predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    pub fn shortcut(status: &mut Status) -> Result<()> {
        std::env::set_current_dir(status.current_tab().directory_of_selected()?)?;
        status.menu.shortcut.update_git_root();
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Shortcut))
    }

    /// Send a signal to parent NVIM process, picking files.
    /// If there's no flagged file, it picks the selected one.
    /// otherwise, flagged files are picked.
    /// If no RPC server were provided at launch time - which may happen for
    /// reasons unknow to me - it does nothing.
    /// It requires the "nvim-send" application to be in $PATH.
    pub fn nvim_filepicker(status: &mut Status) -> Result<()> {
        status.update_nvim_listen_address();
        if status.internal_settings.nvim_server.is_empty() {
            return Ok(());
        };
        let nvim_server = status.internal_settings.nvim_server.clone();
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

    /// Enter the set neovim RPC address mode where the user can type
    /// the RPC address himself
    pub fn set_nvim_server(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::SetNvimAddr))
    }

    /// Move back in history to the last visited directory.
    pub fn back(tab: &mut Tab) -> Result<()> {
        tab.back()
    }

    /// Move to $HOME aka ~.
    pub fn home(status: &mut Status) -> Result<()> {
        let home_cow = shellexpand::tilde("~");
        let home: &str = home_cow.borrow();
        let home_path = path::Path::new(home);
        status.current_tab_mut().cd(home_path)?;
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

    pub fn search_next(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        let Some(searched) = tab.searched.clone() else {
            return Ok(());
        };
        match tab.display_mode {
            Display::Tree => tab.tree.search_first_match(&searched),
            Display::Directory => tab.normal_search_next(&searched),
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
            Edit::Navigate(Navigate::History) => tab.history.prev(),
            Edit::Navigate(navigate) => status.menu.prev(navigate),
            Edit::InputCompleted(_) => status.menu.completion.prev(),
            _ => (),
        };
        status.update_second_pane_for_preview()
    }

    pub fn next_sibling(tab: &mut Tab) -> Result<()> {
        if matches!(tab.display_mode, Display::Tree) {
            tab.tree_next_sibling();
        }
        Ok(())
    }

    pub fn previous_sibling(tab: &mut Tab) -> Result<()> {
        if matches!(tab.display_mode, Display::Tree) {
            tab.tree_prev_sibling();
        }
        Ok(())
    }

    fn move_display_up(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Directory => tab.normal_up_one_row(),
            Display::Preview => tab.preview_page_up(),
            Display::Tree => tab.tree_select_prev()?,
        }
        Ok(())
    }

    fn move_display_down(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Directory => tab.normal_down_one_row(),
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
            Edit::Navigate(Navigate::History) => status.current_tab_mut().history.next(),
            Edit::Navigate(navigate) => status.menu.next(navigate),
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
                Display::Directory => tab.move_to_parent()?,
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
            Edit::Nothing => Self::enter_file(status),
            _ => Ok(()),
        }
    }

    pub fn left_click(status: &mut Status, binds: &Bindings, row: u16, col: u16) -> Result<()> {
        EventAction::select_pane(status, col)?;
        EventAction::click_file(status, row, col, binds)
    }

    pub fn wheel_up(status: &mut Status, col: u16, nb_of_scrolls: u16) -> Result<()> {
        Self::select_pane(status, col)?;
        for _ in 0..nb_of_scrolls {
            Self::move_up(status)?
        }
        Ok(())
    }

    pub fn wheel_down(status: &mut Status, col: u16, nb_of_scrolls: u16) -> Result<()> {
        Self::select_pane(status, col)?;
        for _ in 0..nb_of_scrolls {
            Self::move_down(status)?
        }
        Ok(())
    }

    /// A right click opens a file or a directory.
    pub fn double_click(status: &mut Status, row: u16, col: u16, binds: &Bindings) -> Result<()> {
        if let Ok(()) = EventAction::click_file(status, row, col, binds) {
            EventAction::enter_file(status)?;
        };
        Ok(())
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
                    Display::Directory => tab.normal_go_top(),
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
                    Display::Directory => tab.normal_go_bottom(),
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
            Display::Directory => {
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
            Display::Directory => {
                tab.normal_page_down();
                status.update_second_pane_for_preview()?;
            }
            Display::Preview => tab.preview_page_down(),
            Display::Tree => {
                tab.tree_page_down();
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
        if matches!(status.current_tab().edit_mode, Edit::Nothing) {
            Self::enter_file(status)
        } else {
            LeaveMode::leave_edit_mode(status, binds)
        }
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
        let help = help_string(binds, &status.internal_settings.opener);
        status.skim_find_keybinding_and_run(help)
    }

    /// Copy the filename of the selected file in normal mode.
    pub fn copy_filename(tab: &mut Tab) -> Result<()> {
        if let Display::Directory | Display::Tree = tab.display_mode {
            tab.filename_to_clipboard();
        }
        Ok(())
    }

    /// Copy the filepath of the selected file in normal mode.
    pub fn copy_filepath(tab: &mut Tab) -> Result<()> {
        if let Display::Directory | Display::Tree = tab.display_mode {
            tab.filepath_to_clipboard();
        }
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
        status.set_edit_mode(
            status.index,
            Edit::NeedConfirmation(NeedConfirmation::EmptyTrash),
        )
    }

    /// Open the trash.
    /// Displays a navigable content of the trash.
    /// Each item can be restored or deleted.
    /// Each opening refresh the trash content.
    pub fn trash_open(status: &mut Status) -> Result<()> {
        status.menu.trash.update()?;
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Trash))
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
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::EncryptedDrive))
    }

    /// Enter the Removable Devices mode where the user can mount an MTP device
    pub fn removable_devices(status: &mut Status) -> Result<()> {
        if !is_program_in_path(GIO) {
            log_line!("gio must be installed.");
            return Ok(());
        }
        status.menu.removable_devices = RemovableDevices::from_gio();
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::RemovableDevices))
    }

    /// Open the config file.
    pub fn open_config(status: &mut Status) -> Result<()> {
        match status
            .internal_settings
            .opener
            .open_single(&path::PathBuf::from(
                shellexpand::tilde(CONFIG_PATH).to_string(),
            )) {
            Ok(_) => (),
            Err(e) => log_info!("Error opening {:?}: the config file {}", CONFIG_PATH, e),
        }
        Ok(())
    }

    /// Enter compression mode
    pub fn compress(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Compress))
    }

    /// Enter the context menu mode where the user can choose a basic file action.
    pub fn context(status: &mut Status) -> Result<()> {
        status.menu.context.reset();
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Context))
    }

    /// Enter command mode in which you can type any valid command.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    pub fn command(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Command))?;
        status.menu.completion.reset();
        Ok(())
    }

    /// Execute a custom event on the selected file
    pub fn custom(status: &mut Status, input_string: &str) -> Result<()> {
        status.run_custom_command(input_string)
    }

    /// Enter the remote mount mode where the user can provide an username, an adress and
    /// a mount point to mount a remote device through SSHFS.
    pub fn remote_mount(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Remote))
    }

    /// Click a file at `row`, `col`.
    pub fn click_file(status: &mut Status, row: u16, col: u16, binds: &Bindings) -> Result<()> {
        status.click(row, col, binds)
    }

    /// Select the left or right tab depending on `col`
    pub fn select_pane(status: &mut Status, col: u16) -> Result<()> {
        status.select_tab_from_col(col)
    }

    /// Execute Lazygit in a spawned terminal
    pub fn lazygit(status: &mut Status) -> Result<()> {
        TuiApplications::open_program(status, LAZYGIT)
    }

    /// Execute NCDU in a spawned terminal
    pub fn ncdu(status: &mut Status) -> Result<()> {
        TuiApplications::open_program(status, NCDU)
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
