use std::borrow::Borrow;
use std::path;

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::InMemoryTerm;

use crate::app::{Direction, Focus, Status, Tab};
use crate::common::{
    content_to_clipboard, filename_to_clipboard, filepath_to_clipboard, get_clipboard,
    open_in_current_neovim, set_clipboard, set_current_dir, tilde, CONFIG_PATH,
};
use crate::config::{Bindings, START_FOLDER};
use crate::io::{read_log, Args, External};
use crate::log_info;
use crate::log_line;
use crate::modes::{
    help_string, lsblk_and_udisksctl_installed, nvim_inform_ipc, ContentWindow,
    Direction as FuzzyDirection, Display, FuzzyKind, InputCompleted, InputSimple, LeaveMenu,
    MarkAction, Menu, Navigate, NeedConfirmation, NvimIPCAction, PreviewBuilder, Search,
    Selectable,
};

/// Links events from ratatui to custom actions.
/// It mutates `Status` or its children `Tab`.
pub struct EventAction {}

impl EventAction {
    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's useful to be able to reset the cursor before leaving the application.
    /// If a menu is opened, closes it.
    pub fn quit(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.internal_settings.quit();
        } else {
            status.reset_menu_mode()?;
        }
        Ok(())
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn refresh_view(status: &mut Status) -> Result<()> {
        status.refresh_view()
    }

    /// Refresh the views if files were modified in current directory.
    pub fn refresh_if_needed(status: &mut Status) -> Result<()> {
        status.menu.flagged.remove_non_existant();
        status.tabs[0].refresh_if_needed()?;
        status.tabs[1].refresh_if_needed()
    }

    pub fn resize(status: &mut Status, width: u16, height: u16) -> Result<()> {
        status.resize(width, height)
    }

    /// Leave current mode to normal mode.
    /// Reset the inputs and completion, reset the window, exit the preview.
    pub fn reset_mode(status: &mut Status) -> Result<()> {
        if status.focus.is_file() && status.current_tab().display_mode.is_preview() {
            status.leave_preview()?;
        }
        if matches!(status.current_tab().menu_mode, Menu::Nothing) {
            status.current_tab_mut().reset_visual();
            return Ok(());
        };
        status.leave_menu_mode()?;
        status.menu.input.reset();
        status.menu.completion.reset();
        Ok(())
    }

    /// Toggle between a full display (aka ls -lah) or a simple mode (only the
    /// filenames).
    pub fn toggle_display_full(status: &mut Status) -> Result<()> {
        status.session.toggle_metadata();
        Ok(())
    }

    /// Toggle between dualpane and single pane. Does nothing if the width
    /// is too low to display both panes.
    pub fn toggle_dualpane(status: &mut Status) -> Result<()> {
        status.clear_preview_right();
        status.session.toggle_dual();
        status.select_left();
        Ok(())
    }

    /// Toggle the second pane between preview & normal mode (files).
    pub fn toggle_preview_second(status: &mut Status) -> Result<()> {
        if !status.session.dual() {
            Self::toggle_dualpane(status)?;
            status.session.set_preview();
        } else {
            status.session.toggle_preview();
        }
        if status.session.preview() {
            status.update_second_pane_for_preview()
        } else {
            status.set_menu_mode(1, Menu::Nothing)?;
            status.tabs[1].display_mode = Display::Directory;
            status.tabs[1].refresh_view()
        }
    }

    /// Creates a tree in every mode but "Tree".
    /// In display_mode tree it will exit this view.
    pub fn tree(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        status.current_tab_mut().toggle_tree_mode()?;
        status.refresh_view()
    }

    /// Fold the current node of the tree.
    /// Has no effect on "file" nodes.
    pub fn tree_fold(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let tab = status.current_tab_mut();
        tab.tree.toggle_fold();
        Ok(())
    }

    /// Unfold every child node in the tree.
    /// Recursively explore the tree and unfold every node.
    /// Reset the display.
    pub fn tree_unfold_all(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let tab = status.current_tab_mut();
        tab.tree.unfold_all();
        Ok(())
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node.
    /// Reset the display.
    pub fn tree_fold_all(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let tab = status.current_tab_mut();
        tab.tree.fold_all();
        Ok(())
    }

    /// Toggle the display of flagged files.
    /// Does nothing if a menu is opened.
    pub fn display_flagged(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let menu_mode = &status.current_tab().menu_mode;
        if matches!(menu_mode, Menu::Navigate(Navigate::Flagged)) {
            status.leave_menu_mode()?;
        } else if matches!(menu_mode, Menu::Nothing) {
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::Flagged))?;
        }
        Ok(())
    }

    /// Preview the selected file.
    /// Every file can be previewed. See the `crate::enum::Preview` for
    /// more details on previewinga file.
    /// Does nothing if the directory is empty.
    pub fn preview(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        status.current_tab_mut().make_preview()
    }

    /// Toggle the display of hidden files.
    pub fn toggle_hidden(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() || status.current_tab().display_mode.is_preview() {
            return Ok(());
        }
        status.current_tab_mut().toggle_hidden()
    }

    /// Remove every flag on files in this directory and others.
    pub fn clear_flags(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.menu.flagged.clear();
        }
        Ok(())
    }

    /// Flag all files in the current directory.
    pub fn flag_all(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.flag_all();
        }
        Ok(())
    }

    /// Push every flagged path to the clipboard.
    pub fn flagged_to_clipboard(status: &mut Status) -> Result<()> {
        set_clipboard(status.menu.flagged.content_to_string());
        Ok(())
    }

    /// Replace the currently flagged files by those in clipboard.
    /// Does nothing if the clipboard is empty or can't be read.
    pub fn flagged_from_clipboard(status: &mut Status) -> Result<()> {
        let Some(files) = get_clipboard() else {
            return Ok(());
        };
        if files.is_empty() {
            return Ok(());
        }
        log_info!("clipboard read: {files}");
        status.menu.flagged.replace_by_string(files);
        Ok(())
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn reverse_flags(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.reverse_flags();
        }
        Ok(())
    }

    /// Toggle a single flag and move down one row.
    pub fn toggle_flag(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.toggle_flag_for_selected();
        }
        Ok(())
    }

    /// Enter the rename mode.
    /// Keep a track of the current mode to ensure we rename the correct file.
    /// When we enter rename from a "tree" mode, we'll need to rename the selected file in the tree,
    /// not the selected file in the pathcontent.
    pub fn rename(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Rename)
        ) {
            status.reset_menu_mode()?;
            return Ok(());
        };
        let selected = status.current_tab().current_file()?;
        if selected.path == status.current_tab().directory.path {
            return Ok(());
        }
        if let Some(parent) = status.current_tab().directory.path.parent() {
            if selected.path == std::sync::Arc::from(parent) {
                return Ok(());
            }
        }
        let old_name = &selected.filename;
        status.set_menu_mode(status.index, Menu::InputSimple(InputSimple::Rename))?;
        status.menu.input.replace(old_name);
        Ok(())
    }

    /// Enter a copy paste mode.
    /// A confirmation is asked before copying all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn copy_paste(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::NeedConfirmation(NeedConfirmation::Copy)
        ) {
            status.reset_menu_mode()?;
        } else {
            Self::set_copy_paste(status, NeedConfirmation::Copy)?;
        }
        Ok(())
    }

    /// Enter the 'move' mode.
    /// A confirmation is asked before moving all flagged files to
    /// the current directory.
    /// Does nothing if no file is flagged.
    pub fn cut_paste(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::NeedConfirmation(NeedConfirmation::Move)
        ) {
            status.reset_menu_mode()?;
        } else {
            Self::set_copy_paste(status, NeedConfirmation::Move)?;
        }
        Ok(())
    }

    fn set_copy_paste(status: &mut Status, copy_or_move: NeedConfirmation) -> Result<()> {
        if status.menu.flagged.is_empty() {
            return Ok(());
        }
        status.set_menu_mode(status.index, Menu::NeedConfirmation(copy_or_move))
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn symlink(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
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
        status.set_menu_mode(
            status.index,
            Menu::NeedConfirmation(NeedConfirmation::Delete),
        )
    }

    /// Change to CHMOD mode allowing to edit permissions of a file.
    pub fn chmod(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Chmod)
        ) {
            status.reset_menu_mode()?;
        } else {
            status.set_mode_chmod()?;
        }
        Ok(())
    }

    /// Enter a new node mode.
    fn new_node(status: &mut Status, input_kind: InputSimple) -> Result<()> {
        if !matches!(input_kind, InputSimple::Newdir | InputSimple::Newfile) {
            return Ok(());
        }
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Newdir | InputSimple::Newfile)
        ) {
            status.reset_menu_mode()?;
            return Ok(());
        }
        if matches!(
            status.current_tab().display_mode,
            Display::Directory | Display::Tree
        ) {
            status.set_menu_mode(status.index, Menu::InputSimple(input_kind))?;
        }
        Ok(())
    }

    /// Enter the new dir mode.
    pub fn new_dir(status: &mut Status) -> Result<()> {
        Self::new_node(status, InputSimple::Newdir)
    }

    /// Enter the new file mode.
    pub fn new_file(status: &mut Status) -> Result<()> {
        Self::new_node(status, InputSimple::Newfile)
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
        if tab.display_mode.is_tree() {
            return EventAction::open_file(status);
        };
        if tab.directory.is_empty() {
            return Ok(());
        }
        if tab.directory.is_selected_dir()? {
            tab.go_to_selected_dir()?;
            status.thumbnail_directory_video();
            Ok(())
        } else {
            EventAction::open_file(status)
        }
    }

    /// Execute the selected node if it's a file else enter the directory.
    fn tree_enter_file(status: &mut Status) -> Result<()> {
        let path = status.current_tab().current_file()?.path;
        if path.is_dir() {
            status.current_tab_mut().tree_enter_dir(path)
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
        if !status.focus.is_file() {
            return Ok(());
        }
        if status.menu.flagged.is_empty() {
            status.open_selected_file()
        } else {
            status.open_flagged_files()
        }
    }

    pub fn open_all(status: &mut Status) -> Result<()> {
        status.open_flagged_files()
    }

    /// Enter the execute mode.
    pub fn exec(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Exec)
        ) {
            status.reset_menu_mode()?;
            return Ok(());
        }
        if status.menu.flagged.is_empty() {
            status
                .menu
                .flagged
                .push(status.current_tab().current_file()?.path.to_path_buf());
        }
        status.set_menu_mode(status.index, Menu::InputCompleted(InputCompleted::Exec))
    }

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn sort(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Sort)
        ) {
            status.reset_menu_mode()?;
        }
        status.set_height_for_menu_mode(status.index, Menu::Nothing)?;
        status.tabs[status.index].menu_mode = Menu::Nothing;
        let len = status.menu.len(Menu::Nothing);
        let height = status.second_window_height()?;
        status.menu.window = ContentWindow::new(len, height);
        status.tabs[status.index].menu_mode = Menu::InputSimple(InputSimple::Sort);
        status.set_focus_from_mode();
        Ok(())
    }

    /// Enter the filter mode, where you can filter.
    /// See `crate::modes::Filter` for more details.
    pub fn filter(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Filter)
        ) {
            status.reset_menu_mode()?;
        } else if matches!(
            status.current_tab().display_mode,
            Display::Tree | Display::Directory
        ) {
            status.set_menu_mode(status.index, Menu::InputSimple(InputSimple::Filter))?;
        }
        Ok(())
    }

    /// Enter bulkrename mode, opening a random temp file where the user
    /// can edit the selected filenames.
    /// Once the temp file is saved, those file names are changed.
    pub fn bulk(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        status.bulk_ask_filenames()?;
        Ok(())
    }

    /// Enter the search mode.
    /// Matching items are displayed as you type them.
    pub fn search(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Search)
        ) {
            status.reset_menu_mode()?;
        }
        let tab = status.current_tab_mut();
        tab.search = Search::empty();
        status.set_menu_mode(status.index, Menu::InputCompleted(InputCompleted::Search))
    }

    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    pub fn regex_match(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::RegexMatch)
        ) {
            status.reset_menu_mode()?;
        }
        if matches!(
            status.current_tab().display_mode,
            Display::Tree | Display::Directory
        ) {
            status.set_menu_mode(status.index, Menu::InputSimple(InputSimple::RegexMatch))
        } else {
            Ok(())
        }
    }

    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn help(status: &mut Status, binds: &Bindings) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let help = help_string(binds, &status.internal_settings.opener);
        status.current_tab_mut().set_display_mode(Display::Preview);
        status.current_tab_mut().preview = PreviewBuilder::help(&help);
        let len = status.current_tab().preview.len();
        status.current_tab_mut().window.reset(len);
        Ok(())
    }

    /// Display the last actions impacting the file tree
    pub fn log(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let Ok(log) = read_log() else {
            return Ok(());
        };
        let tab = status.current_tab_mut();
        tab.set_display_mode(Display::Preview);
        tab.preview = PreviewBuilder::log(log);
        tab.window.reset(tab.preview.len());
        tab.preview_go_bottom();
        Ok(())
    }

    /// Enter the cd mode where an user can type a path to jump to.
    pub fn cd(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Cd)
        ) {
            status.reset_menu_mode()?;
        } else {
            status.set_menu_mode(status.index, Menu::InputCompleted(InputCompleted::Cd))?;

            status.tabs[status.index].save_origin_path();
            status.menu.completion.reset();
        }
        Ok(())
    }

    /// Open a new terminal in current directory and current window.
    /// The shell is a fork of current process and will exit if the application
    /// is terminated first.
    pub fn shell(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        set_current_dir(status.current_tab().current_path())?;
        status.internal_settings.disable_display();
        External::open_shell_in_window()?;
        status.internal_settings.enable_display();
        Ok(())
    }

    /// Enter the shell input command mode. The user can type a command which
    /// will be parsed and run.
    pub fn shell_command(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::ShellCommand)
        ) {
            status.reset_menu_mode()?;
        }
        status.set_menu_mode(status.index, Menu::InputSimple(InputSimple::ShellCommand))
    }

    /// Enter the shell menu mode. You can pick a TUI application to be run
    pub fn tui_menu(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::TuiApplication)
        ) {
            status.reset_menu_mode()?;
        } else {
            if status.menu.tui_applications.is_not_set() {
                status.menu.tui_applications.setup();
            }
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::TuiApplication))?;
        }
        Ok(())
    }

    /// Enter the cli info mode. You can pick a Text application to be
    /// displayed/
    pub fn cli_menu(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::CliApplication)
        ) {
            status.reset_menu_mode()?;
        } else {
            if status.menu.cli_applications.is_empty() {
                status.menu.cli_applications.setup();
            }
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::CliApplication))?;
        }
        Ok(())
    }

    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    pub fn history(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::History)
        ) {
            status.reset_menu_mode()?;
        } else if matches!(
            status.current_tab().display_mode,
            Display::Directory | Display::Tree
        ) {
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::History))?;
        }
        Ok(())
    }

    /// Enter Marks new mode, allowing to bind a char to a path.
    pub fn marks_new(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Marks(MarkAction::New))
        ) {
            status.reset_menu_mode()?;
        } else {
            if status.menu.marks.is_empty() {
                status.menu.marks.setup();
            }
            status.set_menu_mode(
                status.index,
                Menu::Navigate(Navigate::Marks(MarkAction::New)),
            )?;
        }
        Ok(())
    }

    /// Enter Marks jump mode, allowing to jump to a marked file.
    pub fn marks_jump(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Marks(MarkAction::Jump))
        ) {
            status.reset_menu_mode()?;
        } else {
            if status.menu.marks.is_empty() {
                status.menu.marks.setup();
            }
            if status.menu.marks.is_empty() {
                return Ok(());
            }
            status.set_menu_mode(
                status.index,
                Menu::Navigate(Navigate::Marks(MarkAction::Jump)),
            )?;
        }
        Ok(())
    }

    /// Enter TempMarks jump mode, allowing to jump to a marked file.
    pub fn temp_marks_jump(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::TempMarks(MarkAction::Jump))
        ) {
            status.reset_menu_mode()?;
        } else {
            status.set_menu_mode(
                status.index,
                Menu::Navigate(Navigate::TempMarks(MarkAction::Jump)),
            )?;
        }
        Ok(())
    }

    /// Enter TempMarks new mode, allowing to bind a char to a path.
    pub fn temp_marks_new(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::TempMarks(MarkAction::New))
        ) {
            status.reset_menu_mode()?;
        } else {
            status.set_menu_mode(
                status.index,
                Menu::Navigate(Navigate::TempMarks(MarkAction::New)),
            )?;
        }
        Ok(())
    }

    /// Enter the shortcut mode, allowing to visit predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    pub fn shortcut(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Shortcut)
        ) {
            status.reset_menu_mode()?;
        } else {
            status.refresh_shortcuts();
            set_current_dir(status.current_tab().directory_of_selected()?)?;
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::Shortcut))?;
        }
        Ok(())
    }

    /// Send a signal to parent NVIM process, picking files.
    /// If there's no flagged file, it picks the selected one.
    /// otherwise, flagged files are picked.
    /// If no RPC server were provided at launch time - which may happen for
    /// reasons unknow to me - it does nothing.
    pub fn nvim_filepicker(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        status.update_nvim_listen_address();
        if status.internal_settings.nvim_server.is_empty() {
            return Ok(());
        };
        let nvim_server = &status.internal_settings.nvim_server;
        if status.menu.flagged.is_empty() {
            let Ok(fileinfo) = status.current_tab().current_file() else {
                return Ok(());
            };
            open_in_current_neovim(&fileinfo.path, nvim_server);
        } else {
            for file_path in status.menu.flagged.content.iter() {
                open_in_current_neovim(file_path, nvim_server)
            }
        }

        Ok(())
    }

    /// Enter the set neovim RPC address mode where the user can type
    /// the RPC address himself
    pub fn set_nvim_server(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::SetNvimAddr)
        ) {
            status.reset_menu_mode()?;
            return Ok(());
        };
        status.set_menu_mode(status.index, Menu::InputSimple(InputSimple::SetNvimAddr))
    }

    /// Move back in history to the last visited directory.
    pub fn back(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        status.current_tab_mut().back()?;
        status.update_second_pane_for_preview()
    }

    /// Move to $HOME aka ~.
    pub fn home(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let home_cow = tilde("~");
        let home: &str = home_cow.borrow();
        let home_path = path::Path::new(home);
        status.current_tab_mut().cd(home_path)?;
        status.update_second_pane_for_preview()
    }

    pub fn go_root(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        let root_path = std::path::PathBuf::from("/");
        status.current_tab_mut().cd(&root_path)?;
        status.update_second_pane_for_preview()
    }

    pub fn go_start(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        status
            .current_tab_mut()
            .cd(START_FOLDER.get().context("Start folder should be set")?)?;
        status.update_second_pane_for_preview()
    }

    pub fn search_next(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        match status.current_tab().display_mode {
            Display::Tree => status.tabs[status.index]
                .search
                .tree(&mut status.tabs[status.index].tree),
            Display::Directory => status.current_tab_mut().directory_search_next(),
            Display::Preview | Display::Fuzzy => {
                return Ok(());
            }
        }
        status.refresh_status()?;
        status.update_second_pane_for_preview()?;
        Ok(())
    }

    /// Move up one row in modes allowing movement.
    /// Does nothing if the selected item is already the first in list.
    pub fn move_up(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            Self::move_display_up(status)?;
        } else {
            let tab = status.current_tab_mut();
            match tab.menu_mode {
                Menu::Nothing => Self::move_display_up(status)?,
                Menu::Navigate(Navigate::History) => tab.history.prev(),
                Menu::Navigate(navigate) => status.menu.prev(navigate),
                Menu::InputCompleted(input_completed) => {
                    status.menu.completion_prev(input_completed);
                    if matches!(input_completed, InputCompleted::Search) {
                        status.follow_search()?;
                    }
                }
                Menu::NeedConfirmation(need_confirmation)
                    if need_confirmation.use_flagged_files() =>
                {
                    status.menu.prev(Navigate::Flagged)
                }
                Menu::NeedConfirmation(NeedConfirmation::EmptyTrash) => {
                    status.menu.prev(Navigate::Trash)
                }
                Menu::NeedConfirmation(NeedConfirmation::BulkAction) => {
                    status.menu.bulk.prev();
                    status.menu.window.scroll_to(status.menu.bulk.index())
                }
                _ => (),
            };
        }
        status.update_second_pane_for_preview()
    }

    /// Special move to the next "thing".
    /// if we're in tree mode, focusing a file, it's the next sibling (= node of the same level sharing parent)
    /// if we're inputing something, it's the next history result
    pub fn next_thing(status: &mut Status) -> Result<()> {
        if status.current_tab().display_mode.is_tree() && status.focus.is_file() {
            status.current_tab_mut().tree_next_sibling();
        } else {
            status.input_history_prev()?;
        }
        Ok(())
    }

    /// Special move to the previous "thing".
    /// if we're in tree mode, focusing a file, it's the previous sibling (= node of the same level sharing parent)
    /// if we're inputing something, it's the previous history result
    pub fn previous_thing(status: &mut Status) -> Result<()> {
        if status.current_tab().display_mode.is_tree() && status.focus.is_file() {
            status.current_tab_mut().tree_prev_sibling();
        } else {
            status.input_history_next()?;
        }
        Ok(())
    }

    fn move_display_up(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Directory => {
                tab.normal_up_one_row();
                status.toggle_flag_visual();
            }
            Display::Preview => tab.preview_page_up(),
            Display::Tree => {
                tab.tree_select_prev();
                status.toggle_flag_visual();
            }
            Display::Fuzzy => status.fuzzy_navigate(FuzzyDirection::Up)?,
        }
        Ok(())
    }

    fn move_display_down(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        match tab.display_mode {
            Display::Directory => {
                tab.normal_down_one_row();
                status.toggle_flag_visual();
            }
            Display::Preview => tab.preview_page_down(),
            Display::Tree => {
                tab.tree_select_next();
                status.toggle_flag_visual()
            }
            Display::Fuzzy => status.fuzzy_navigate(FuzzyDirection::Down)?,
        }
        Ok(())
    }
    /// Move down one row in modes allowing movements.
    /// Does nothing if the user is already at the bottom.
    pub fn move_down(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            Self::move_display_down(status)?
        } else {
            match status.current_tab_mut().menu_mode {
                Menu::Nothing => Self::move_display_down(status)?,
                Menu::Navigate(Navigate::History) => status.current_tab_mut().history.next(),
                Menu::Navigate(navigate) => status.menu.next(navigate),
                Menu::InputCompleted(input_completed) => {
                    status.menu.completion_next(input_completed);
                    if matches!(input_completed, InputCompleted::Search) {
                        status.follow_search()?;
                    }
                }
                Menu::NeedConfirmation(need_confirmation)
                    if need_confirmation.use_flagged_files() =>
                {
                    status.menu.next(Navigate::Flagged)
                }
                Menu::NeedConfirmation(NeedConfirmation::EmptyTrash) => {
                    status.menu.next(Navigate::Trash)
                }
                Menu::NeedConfirmation(NeedConfirmation::BulkAction) => {
                    status.menu.bulk.next();
                    status.menu.window.scroll_to(status.menu.bulk.index())
                }
                _ => (),
            };
        }
        status.update_second_pane_for_preview()
    }

    /// Move to parent in normal mode,
    /// move left one char in mode requiring text input.
    pub fn move_left(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            Self::file_move_left(status.current_tab_mut())?;
        } else {
            let tab = status.current_tab_mut();
            match tab.menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => {
                    status.menu.input.cursor_left();
                }
                Menu::Nothing => Self::file_move_left(tab)?,
                Menu::Navigate(Navigate::Cloud) => status.cloud_move_to_parent()?,
                _ => (),
            }
        }
        status.update_second_pane_for_preview()
    }

    fn file_move_left(tab: &mut Tab) -> Result<()> {
        match tab.display_mode {
            Display::Directory => tab.move_to_parent()?,
            Display::Tree => tab.tree_select_parent()?,
            _ => (),
        };
        Ok(())
    }

    /// Move to child if any or open a regular file in normal mode.
    /// Move the cursor one char to right in mode requiring text input.
    pub fn move_right(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            Self::enter_file(status)
        } else {
            let tab: &mut Tab = status.current_tab_mut();
            match tab.menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => {
                    status.menu.input.cursor_right();
                    Ok(())
                }
                Menu::Navigate(Navigate::Cloud) => status.cloud_enter_file_or_dir(),
                Menu::Nothing => Self::enter_file(status),
                _ => Ok(()),
            }
        }
    }

    pub fn focus_follow_mouse(status: &mut Status, row: u16, col: u16) -> Result<()> {
        status.set_focus_from_pos(row, col)?;
        Ok(())
    }

    /// Click a file at `row`, `col`. Gives the focus to the window container.
    fn click(status: &mut Status, binds: &Bindings, row: u16, col: u16) -> Result<()> {
        status.click(binds, row, col)
    }

    /// Left Click a file at `row`, `col`. Gives the focus to the window container.
    pub fn left_click(status: &mut Status, binds: &Bindings, row: u16, col: u16) -> Result<()> {
        Self::click(status, binds, row, col)
    }

    /// Right click gives focus to the window and open the context menu
    pub fn right_click(status: &mut Status, binds: &Bindings, row: u16, col: u16) -> Result<()> {
        Self::click(status, binds, row, col)?;
        Self::context(status)
    }

    /// Wheel up moves the display up
    pub fn wheel_up(status: &mut Status, row: u16, col: u16) -> Result<()> {
        status.set_focus_from_pos(row, col)?;
        Self::move_up(status)
    }

    /// Wheel down moves the display down
    pub fn wheel_down(status: &mut Status, row: u16, col: u16) -> Result<()> {
        status.set_focus_from_pos(row, col)?;
        Self::move_down(status)
    }

    /// A middle_click click opens a file or execute a menu item
    pub fn middle_click(status: &mut Status, binds: &Bindings, row: u16, col: u16) -> Result<()> {
        if Self::click(status, binds, row, col).is_ok() {
            if status.focus.is_file() {
                Self::enter_file(status)?;
            } else {
                LeaveMenu::leave_menu(status, binds)?;
            }
        };
        Ok(())
    }

    /// Delete a char to the left in modes allowing edition.
    pub fn backspace(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            return Ok(());
        }
        match status.current_tab().menu_mode {
            Menu::Navigate(Navigate::Marks(_)) => {
                status.menu.marks.remove_selected()?;
            }
            Menu::Navigate(Navigate::TempMarks(_)) => {
                status.menu.temp_marks.erase_current_mark();
            }
            Menu::InputSimple(_) | Menu::InputCompleted(_) => {
                status.menu.input.delete_char_left();
            }
            _ => (),
        }
        Ok(())
    }

    /// When files are focused, try to delete the flagged files (or selected if no file is flagged)
    /// Inform through the output socket if any was provided in console line arguments.
    /// When edit window is focused, delete all chars to the right in mode allowing edition.
    pub fn delete(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            Self::delete_file(status)
        } else {
            match status.current_tab_mut().menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => {
                    status.menu.input.delete_chars_right();
                    Ok(())
                }
                _ => Ok(()),
            }
        }
    }

    pub fn delete_line(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.sync_tabs(Direction::RightToLeft)?;
        }
        match status.current_tab_mut().menu_mode {
            Menu::InputSimple(_) => {
                status.menu.input.delete_line();
            }
            Menu::InputCompleted(_) => {
                status.menu.input.delete_line();
                status.menu.completion_reset();
            }
            _ => (),
        }
        Ok(())
    }

    /// Delete one word to the left in menus with input
    pub fn delete_left(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            return Ok(());
        }
        match status.current_tab_mut().menu_mode {
            Menu::InputSimple(_) => {
                status.menu.input.delete_left();
            }
            Menu::InputCompleted(_) => {
                status.menu.input.delete_left();
                status.menu.completion_reset();
            }
            _ => (),
        }
        Ok(())
    }

    /// Move to leftmost char in mode allowing edition.
    pub fn key_home(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            let tab = status.current_tab_mut();
            match tab.display_mode {
                Display::Directory => tab.normal_go_top(),
                Display::Preview => tab.preview_go_top(),
                Display::Tree => tab.tree_go_to_root()?,
                Display::Fuzzy => status.fuzzy_start()?,
            };
            status.update_second_pane_for_preview()
        } else {
            match status.current_tab().menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => status.menu.input.cursor_start(),
                Menu::Navigate(navigate) => status.menu.set_index(0, navigate),
                _ => (),
            }
            Ok(())
        }
    }

    /// Move to the bottom in any mode.
    pub fn end(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            let tab = status.current_tab_mut();
            match tab.display_mode {
                Display::Directory => tab.normal_go_bottom(),
                Display::Preview => tab.preview_go_bottom(),
                Display::Tree => tab.tree_go_to_bottom_leaf(),
                Display::Fuzzy => status.fuzzy_end()?,
            };
            status.update_second_pane_for_preview()?;
        } else {
            match status.current_tab().menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => status.menu.input.cursor_end(),
                Menu::Navigate(navigate) => status.menu.select_last(navigate),
                _ => (),
            }
        }
        Ok(())
    }

    /// Move up 10 lines in normal mode and preview.
    pub fn page_up(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            Self::file_page_up(status)?;
        } else {
            let tab = status.current_tab_mut();
            match tab.menu_mode {
                Menu::Nothing => Self::file_page_up(status)?,
                Menu::Navigate(navigate) => status.menu.page_up(navigate),
                Menu::InputCompleted(input_completed) => {
                    for _ in 0..10 {
                        status.menu.completion_prev(input_completed)
                    }
                    if matches!(input_completed, InputCompleted::Search) {
                        status.follow_search()?;
                    }
                }
                Menu::NeedConfirmation(need_confirmation)
                    if need_confirmation.use_flagged_files() =>
                {
                    for _ in 0..10 {
                        status.menu.prev(Navigate::Flagged)
                    }
                }
                Menu::NeedConfirmation(NeedConfirmation::EmptyTrash) => {
                    for _ in 0..10 {
                        status.menu.prev(Navigate::Trash)
                    }
                }
                Menu::NeedConfirmation(NeedConfirmation::BulkAction) => {
                    for _ in 0..10 {
                        status.menu.bulk.prev()
                    }
                    status.menu.window.scroll_to(status.menu.bulk.index())
                }
                _ => (),
            };
        }
        Ok(())
    }

    fn file_page_up(status: &mut Status) -> Result<()> {
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
            Display::Fuzzy => status.fuzzy_navigate(FuzzyDirection::PageUp)?,
        };
        Ok(())
    }

    /// Move down 10 lines in normal & preview mode.
    pub fn page_down(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            Self::file_page_down(status)?;
        } else {
            let tab = status.current_tab_mut();
            match tab.menu_mode {
                Menu::Nothing => Self::file_page_down(status)?,
                Menu::Navigate(navigate) => status.menu.page_down(navigate),
                Menu::InputCompleted(input_completed) => {
                    for _ in 0..10 {
                        status.menu.completion_next(input_completed)
                    }
                    if matches!(input_completed, InputCompleted::Search) {
                        status.follow_search()?;
                    }
                }
                Menu::NeedConfirmation(need_confirmation)
                    if need_confirmation.use_flagged_files() =>
                {
                    for _ in 0..10 {
                        status.menu.next(Navigate::Flagged)
                    }
                }
                Menu::NeedConfirmation(NeedConfirmation::EmptyTrash) => {
                    for _ in 0..10 {
                        status.menu.next(Navigate::Trash)
                    }
                }
                Menu::NeedConfirmation(NeedConfirmation::BulkAction) => {
                    for _ in 0..10 {
                        status.menu.bulk.next()
                    }
                    status.menu.window.scroll_to(status.menu.bulk.index())
                }
                _ => (),
            };
        }
        Ok(())
    }

    fn file_page_down(status: &mut Status) -> Result<()> {
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
            Display::Fuzzy => status.fuzzy_navigate(FuzzyDirection::PageDown)?,
        };
        Ok(())
    }

    /// Execute the mode.
    /// In modes requiring confirmation or text input, it will execute the
    /// related action.
    /// In normal mode, it will open the file.
    /// Reset to normal mode afterwards.
    pub fn enter(status: &mut Status, binds: &Bindings) -> Result<()> {
        if status.focus.is_file() {
            Self::enter_file(status)
        } else {
            LeaveMenu::leave_menu(status, binds)
        }
    }

    /// Change tab in normal mode with dual pane displayed,
    /// insert a completion in modes allowing completion.
    pub fn tab(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.next()
        } else if let Menu::InputCompleted(input_completed) = status.current_tab_mut().menu_mode {
            status.complete_tab(input_completed)?;
        }
        Ok(())
    }

    /// Start a fuzzy find with skim.
    pub fn fuzzyfind(status: &mut Status) -> Result<()> {
        status.force_clear();
        status.fuzzy_init(FuzzyKind::File);
        status.current_tab_mut().set_display_mode(Display::Fuzzy);
        status.fuzzy_find_files()?;
        status.update_second_pane_for_preview()
    }

    /// Start a fuzzy find for a specific line with skim.
    pub fn fuzzyfind_line(status: &mut Status) -> Result<()> {
        status.force_clear();
        status.fuzzy_init(FuzzyKind::Line);
        status.current_tab_mut().set_display_mode(Display::Fuzzy);
        status.fuzzy_find_lines()
    }

    /// Start a fuzzy find for a keybinding with skim.
    pub fn fuzzyfind_help(status: &mut Status, binds: &Bindings) -> Result<()> {
        status.fuzzy_init(FuzzyKind::Action);
        status.current_tab_mut().set_display_mode(Display::Fuzzy);
        let help = help_string(binds, &status.internal_settings.opener);
        status.fuzzy_help(help)?;
        Ok(())
    }

    /// Copy the content of the selected text file in normal mode.
    pub fn copy_content(status: &Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        match status.current_tab().display_mode {
            Display::Tree | Display::Directory => {
                let Ok(file_info) = status.current_tab().current_file() else {
                    return Ok(());
                };
                content_to_clipboard(&file_info.path);
            }
            _ => return Ok(()),
        }
        Ok(())
    }
    /// Copy the filename of the selected file in normal mode.
    pub fn copy_filename(status: &Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        match status.current_tab().display_mode {
            Display::Tree | Display::Directory => {
                let Ok(file_info) = status.current_tab().current_file() else {
                    return Ok(());
                };
                filename_to_clipboard(&file_info.path);
            }
            _ => return Ok(()),
        }
        Ok(())
    }

    /// Copy the filepath of the selected file in normal mode.
    pub fn copy_filepath(status: &Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        match status.current_tab().display_mode {
            Display::Tree | Display::Directory => {
                let Ok(file_info) = status.current_tab().current_file() else {
                    return Ok(());
                };
                filepath_to_clipboard(&file_info.path);
            }
            _ => return Ok(()),
        }
        Ok(())
    }

    /// Move flagged files to the trash directory.
    /// If no file is flagged, flag the selected file and move it to trash.
    /// Inform the listener through the output_socket if any was provided in console line arguments.
    /// More information in the trash mod itself.
    /// We can't trash file which aren't mounted in the same partition.
    /// If the file is mounted on the $topdir of the trash (aka the $HOME mount point),
    /// it is moved there.
    /// Else, nothing is done.
    pub fn trash_move_file(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        if status.menu.flagged.is_empty() {
            Self::toggle_flag(status)?;
        }

        status.menu.trash.update()?;
        let output_socket = Args::parse().output_socket;
        for flagged in status.menu.flagged.content.iter() {
            if status.menu.trash.trash(flagged).is_ok() {
                if let Some(output_socket) = &output_socket {
                    nvim_inform_ipc(output_socket, NvimIPCAction::DELETE(flagged))?;
                }
            }
        }
        status.menu.flagged.clear();
        status.current_tab_mut().refresh_view()?;
        Ok(())
    }

    /// Ask the user if he wants to empty the trash.
    /// It requires a confimation before doing anything
    pub fn trash_empty(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::NeedConfirmation(NeedConfirmation::EmptyTrash)
        ) {
            status.reset_menu_mode()?;
        } else {
            status.menu.trash.update()?;
            status.set_menu_mode(
                status.index,
                Menu::NeedConfirmation(NeedConfirmation::EmptyTrash),
            )?;
        }
        Ok(())
    }

    /// Open the trash.
    /// Displays a navigable content of the trash.
    /// Each item can be restored or deleted.
    /// Each opening refresh the trash content.
    pub fn trash_open(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Trash)
        ) {
            status.reset_menu_mode()?;
        } else {
            status.menu.trash.update()?;
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::Trash))?;
        }
        Ok(())
    }

    pub fn trash_restore(status: &mut Status) -> Result<()> {
        LeaveMenu::trash(status)
    }

    /// Open the config file.
    pub fn open_config(status: &mut Status) -> Result<()> {
        if !status.focus.is_file() {
            return Ok(());
        }
        match status.open_single_file(&path::PathBuf::from(tilde(CONFIG_PATH).to_string())) {
            Ok(_) => log_line!("Opened the config file {CONFIG_PATH}"),
            Err(e) => log_info!("Error opening {:?}: the config file {}", CONFIG_PATH, e),
        }
        Ok(())
    }

    /// Enter compression mode
    pub fn compress(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Compress)
        ) {
            status.reset_menu_mode()?;
        } else {
            if status.menu.compression.is_empty() {
                status.menu.compression.setup();
            }
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::Compress))?;
        }
        Ok(())
    }

    /// Enter the context menu mode where the user can choose a basic file action.
    pub fn context(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Context)
        ) {
            status.reset_menu_mode()?;
        } else {
            if status.menu.context.is_empty() {
                status.menu.context.setup();
            } else {
                status.menu.context.reset();
            }
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::Context))?;
        }
        Ok(())
    }

    /// Enter action mode in which you can type any valid action.
    /// Some action does nothing as they require to be executed from a specific context.
    pub fn action(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Action)
        ) {
            status.reset_menu_mode()?;
        } else {
            status.set_menu_mode(status.index, Menu::InputCompleted(InputCompleted::Action))?;
            status.menu.completion.reset();
        }
        Ok(())
    }

    /// Enter the remote mount mode where the user can provide an username, an adress and
    /// a mount point to mount a remote device through SSHFS.
    pub fn remote_mount(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Remote)
        ) {
            status.reset_menu_mode()?;
        }
        status.set_menu_mode(status.index, Menu::InputSimple(InputSimple::Remote))
    }

    pub fn cloud_drive(status: &mut Status) -> Result<()> {
        status.cloud_open()
    }

    /// Select the left or right tab depending on `col`
    pub fn select_pane(status: &mut Status, col: u16) -> Result<()> {
        status.select_tab_from_col(col)
    }

    pub fn focus_go_left(status: &mut Status) -> Result<()> {
        match status.focus {
            Focus::LeftMenu | Focus::LeftFile => (),
            Focus::RightFile => {
                status.index = 0;
                status.focus = Focus::LeftFile;
            }
            Focus::RightMenu => {
                status.index = 0;
                if matches!(status.tabs[0].menu_mode, Menu::Nothing) {
                    status.focus = Focus::LeftFile;
                } else {
                    status.focus = Focus::LeftMenu;
                }
            }
        }
        Ok(())
    }

    pub fn focus_go_right(status: &mut Status) -> Result<()> {
        match status.focus {
            Focus::RightMenu | Focus::RightFile => (),
            Focus::LeftFile => {
                status.index = 1;
                status.focus = Focus::RightFile;
            }
            Focus::LeftMenu => {
                status.index = 1;
                if matches!(status.tabs[1].menu_mode, Menu::Nothing) {
                    status.focus = Focus::RightFile;
                } else {
                    status.focus = Focus::RightMenu;
                }
            }
        }
        Ok(())
    }

    pub fn focus_go_down(status: &mut Status) -> Result<()> {
        match status.focus {
            Focus::RightMenu | Focus::LeftMenu => (),
            Focus::LeftFile => {
                if !matches!(status.tabs[0].menu_mode, Menu::Nothing) {
                    status.focus = Focus::LeftMenu;
                }
            }
            Focus::RightFile => {
                if !matches!(status.tabs[1].menu_mode, Menu::Nothing) {
                    status.focus = Focus::RightMenu;
                }
            }
        }
        Ok(())
    }

    pub fn focus_go_up(status: &mut Status) -> Result<()> {
        match status.focus {
            Focus::LeftFile | Focus::RightFile => (),
            Focus::LeftMenu => status.focus = Focus::LeftFile,
            Focus::RightMenu => status.focus = Focus::RightFile,
        }
        Ok(())
    }

    pub fn sync_ltr(status: &mut Status) -> Result<()> {
        if status.focus.is_file() {
            status.sync_tabs(Direction::LeftToRight)?;
        }
        Ok(())
    }

    pub fn bulk_confirm(status: &mut Status) -> Result<()> {
        status.bulk_execute()
    }

    pub fn file_copied(status: &mut Status) -> Result<()> {
        log_info!(
            "file copied - pool: {pool:?}",
            pool = status.internal_settings.copy_file_queue
        );
        status.internal_settings.copy_file_remove_head()?;
        if status.internal_settings.copy_file_queue.is_empty() {
            status.internal_settings.unset_copy_progress()
        } else {
            status.copy_next_file_in_queue()?;
        }
        Ok(())
    }

    pub fn display_copy_progress(status: &mut Status, content: InMemoryTerm) -> Result<()> {
        status.internal_settings.store_copy_progress(content);
        Ok(())
    }

    pub fn check_preview_fuzzy_tick(status: &mut Status) -> Result<()> {
        status.fuzzy_tick();
        status.check_preview()
    }

    pub fn visual(status: &mut Status) -> Result<()> {
        status.current_tab_mut().toggle_visual();
        status.toggle_flag_visual();

        Ok(())
    }

    /// Open the mount menu
    pub fn mount(status: &mut Status) -> Result<()> {
        if matches!(
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Mount)
        ) {
            status.reset_menu_mode()?;
        } else if lsblk_and_udisksctl_installed() {
            status.menu.mount.update(status.internal_settings.disks())?;
            status.set_menu_mode(status.index, Menu::Navigate(Navigate::Mount))?;
        }
        Ok(())
    }

    /// Execute a custom event on the selected file
    pub fn custom(status: &mut Status, input_string: &str) -> Result<()> {
        status.run_custom_command(input_string)
    }

    /// Parse and execute the received IPC message.
    pub fn parse_rpc(status: &mut Status, ipc_msg: String) -> Result<()> {
        status.parse_ipc(ipc_msg)
    }
}
