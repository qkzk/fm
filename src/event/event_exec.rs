use anyhow::{Context, Result};
use crate::common::{is_program_in_path, open_in_current_neovim};
use crate::common::{CONFIG_PATH, GIO};
use crate::config::Bindings;
use crate::config::START_FOLDER;
use crate::io::execute_without_output_with_path;
use crate::io::SpecificCommand;
use crate::modes::help_string;
use crate::modes::ContentWindow;
use crate::modes::LeaveMode;
use crate::modes::Preview;
use crate::modes::TuiApplications;
    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's usefull to reset the cursor before leaving the application.
    pub fn quit(status: &mut Status) -> Result<()> {
        status.internal_settings.must_quit = true;
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
        let tab = &mut status.tabs[status.index];
        if matches!(tab.display_mode, Display::Preview) {
            tab.set_display_mode(Display::Directory);
        }
        if tab.reset_edit_mode() {
            status.menu.completion.reset();
            tab.refresh_view()
        } else {
            tab.refresh_params()
        }
    }

    /// Toggle between a full display (aka ls -lah) or a simple mode (only the
    /// filenames).
    pub fn toggle_display_full(status: &mut Status) -> Result<()> {
        status.display_settings.metadata = !status.display_settings.metadata;
    /// Toggle between dualpane and single pane. Does nothing if the width
    /// is too low to display both panes.
    pub fn toggle_dualpane(status: &mut Status) -> Result<()> {
        status.display_settings.dual = !status.display_settings.dual;
        status.select_left();
    /// Toggle the second pane between preview & normal mode (files).
    pub fn toggle_preview_second(status: &mut Status) -> Result<()> {
        status.display_settings.preview = !status.display_settings.preview;
        if status.display_settings.preview {
            status.set_second_pane_for_preview()?;
        } else {
            status.tabs[1].reset_edit_mode();
            status.tabs[1].refresh_view()?;
        }
        Ok(())
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
    /// Unfold every child node in the tree.
    /// Recursively explore the tree and unfold every node.
    /// Reset the display.
    pub fn tree_unfold_all(tab: &mut Tab) -> Result<()> {
        tab.tree.unfold_all();
        Ok(())
    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node.
    /// Reset the display.
    pub fn tree_fold_all(tab: &mut Tab) -> Result<()> {
        tab.tree.fold_all();
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
    /// Flag all files in the current directory.
    pub fn flag_all(status: &mut Status) -> Result<()> {
        status.flag_all();

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn reverse_flags(status: &mut Status) -> Result<()> {
        status.reverse_flags();
        Ok(())
    /// Toggle a single flag and move down one row.
    pub fn toggle_flag(status: &mut Status) -> Result<()> {
        status.toggle_flag_for_selected();
    /// Enter the rename mode.
    /// Keep a track of the current mode to ensure we rename the correct file.
    /// When we enter rename from a "tree" mode, we'll need to rename the selected file in the tree,
    /// not the selected file in the pathcontent.
    pub fn rename(status: &mut Status) -> Result<()> {
        let selected = status.current_tab().current_file()?;
        let sel_path = selected.path;
        if sel_path == status.current_tab().directory.path {
            return Ok(());
        if let Some(parent) = status.current_tab().directory.path.parent() {
            if sel_path == std::rc::Rc::from(parent) {
                return Ok(());
            }
        let old_name = &selected.filename;
        status.menu.input.replace(old_name);
        status
            .current_tab_mut()
            .set_edit_mode(Edit::InputSimple(InputSimple::Rename));
        Ok(())
        if status.menu.flagged.is_empty() {
            .current_tab_mut()
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
        status
            .current_tab_mut()
            .set_edit_mode(Edit::NeedConfirmation(NeedConfirmation::Delete));
        Ok(())
    }

    /// Change to CHMOD mode allowing to edit permissions of a file.
    pub fn chmod(status: &mut Status) -> Result<()> {
        status.set_mode_chmod()
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

    /// Enter the sort mode, allowing the user to select a sort method.
    pub fn sort(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Sort));
        Ok(())
    /// Enter the filter mode, where you can filter.
    /// See `crate::modes::Filter` for more details.
    pub fn filter(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Filter));
    /// Enter JUMP mode, allowing to jump to any flagged file.
    /// Does nothing if no file is flagged.
    pub fn jump(status: &mut Status) -> Result<()> {
        if !status.menu.flagged.is_empty() {
            status.menu.flagged.index = 0;
            status
                .current_tab_mut()
                .set_edit_mode(Edit::Navigate(Navigate::Jump))
        }
    /// Enter bulkrename mode, opening a random temp file where the user
    /// can edit the selected filenames.
    /// Once the temp file is saved, those file names are changed.
    pub fn bulk(status: &mut Status) -> Result<()> {
        status.menu.init_bulk();
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::Bulk));
    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn help(status: &mut Status, binds: &Bindings) -> Result<()> {
        let help = help_string(binds, &status.internal_settings.opener)?;
        status.current_tab_mut().set_display_mode(Display::Preview);
        status.current_tab_mut().preview = Preview::help(&help);
        let len = status.current_tab().preview.len();
        status.current_tab_mut().window.reset(len);
    /// Display the last actions impacting the file tree
    pub fn log(tab: &mut Tab) -> Result<()> {
        let log = read_log()?;
        tab.set_display_mode(Display::Preview);
        tab.preview = Preview::log(log);
        tab.window.reset(tab.preview.len());
        tab.preview_go_bottom();
    pub fn goto(status: &mut Status) -> Result<()> {
        status
            .current_tab_mut()
            .set_edit_mode(Edit::InputCompleted(InputCompleted::Goto));
        status.menu.completion.reset();
        let tab = status.current_tab();
        execute_without_output_with_path(&status.internal_settings.opener.terminal, path, None)?;
    pub fn tui_menu(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::Navigate(Navigate::TuiApplication));
    pub fn cli_menu(status: &mut Status) -> Result<()> {
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::CliApplication));
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

    pub fn shortcut(status: &mut Status) -> Result<()> {
        std::env::set_current_dir(status.current_tab().directory_of_selected()?)?;
        status.menu.shortcut.update_git_root();
        status
            .current_tab_mut()
            .set_edit_mode(Edit::Navigate(Navigate::Shortcut));
        if status.internal_settings.nvim_server.is_empty() {
        let nvim_server = status.internal_settings.nvim_server.clone();
        if status.menu.flagged.is_empty() {
            let Ok(fileinfo) = status.current_tab().current_file() else {
            let flagged = status.menu.flagged.content.clone();
        status.current_tab_mut().cd(&path)?;
        status.current_tab_mut().cd(&root_path)?;
        status.current_tab_mut().cd(&START_FOLDER)?;
        let tab = status.current_tab_mut();
            Display::Directory => tab.normal_search_next(&searched),
        let tab = status.current_tab_mut();
            Edit::Navigate(Navigate::Jump) => status.menu.flagged.prev(),
            Edit::Navigate(Navigate::Trash) => status.menu.trash.prev(),
            Edit::Navigate(Navigate::Shortcut) => status.menu.shortcut.prev(),
            Edit::Navigate(Navigate::Marks(_)) => status.menu.marks.prev(),
            Edit::Navigate(Navigate::Compress) => status.menu.compression.prev(),
            Edit::Navigate(Navigate::Bulk) => status.menu.bulk_prev(),
            Edit::Navigate(Navigate::TuiApplication) => status.menu.tui_applications.prev(),
            Edit::Navigate(Navigate::CliApplication) => status.menu.cli_applications.prev(),
            Edit::Navigate(Navigate::EncryptedDrive) => status.menu.encrypted_devices.prev(),
            Edit::InputCompleted(_) => status.menu.completion.prev(),
        let tab = status.current_tab_mut();
            Display::Directory => tab.normal_up_one_row(),
        let tab = status.current_tab_mut();
            Display::Directory => tab.normal_down_one_row(),
        match status.current_tab_mut().edit_mode {
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
        let tab = status.current_tab_mut();
                status.menu.input.cursor_left();
                Display::Directory => tab.move_to_parent()?,
        let tab: &mut Tab = status.current_tab_mut();
                status.menu.input.cursor_right();
            Edit::Nothing => Self::enter_file(status),
    pub fn left_click(status: &mut Status, binds: &Bindings, row: u16, col: u16) -> Result<()> {
        EventAction::select_pane(status, col)?;
        if ContentWindow::is_row_in_header(row) {
            EventAction::click_first_line(col, status, binds)?;
        } else {
            let _ = EventAction::click_file(status, row, col);
        }
        Ok(())
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
    pub fn right_click(status: &mut Status, row: u16, col: u16) -> Result<()> {
        if let Ok(()) = EventAction::click_file(status, row, col) {
            EventAction::enter_file(status)?;
        };
        Ok(())
    }

    pub fn backspace(status: &mut Status) -> Result<()> {
        match status.current_tab().edit_mode {
                status.menu.input.delete_char_left();
        match status.current_tab_mut().edit_mode {
                status.menu.input.delete_chars_right();
        let tab = status.current_tab_mut();
                    Display::Directory => tab.normal_go_top(),
            _ => status.menu.input.cursor_start(),
        let tab = status.current_tab_mut();
                    Display::Directory => tab.normal_go_bottom(),
            _ => status.menu.input.cursor_end(),
        let tab = status.current_tab_mut();
            Display::Directory => {
        let tab = status.current_tab_mut();
            Display::Directory => {
    pub fn enter(status: &mut Status, binds: &Bindings) -> Result<()> {
        if matches!(status.current_tab().edit_mode, Edit::Nothing) {
            Self::enter_file(status)
        } else {
            LeaveMode::leave_edit_mode(status, binds)
        match status.current_tab_mut().edit_mode {
            Edit::InputCompleted(_) => status
                .menu
                .input
                .replace(status.menu.completion.current_proposition()),
        if matches!(status.current_tab().edit_mode, Edit::Nothing) {
    pub fn fuzzyfind_help(status: &mut Status, binds: &Bindings) -> Result<()> {
        let help = help_string(binds, &status.internal_settings.opener)?;
        status.skim_find_keybinding_and_run(help)
        if let Display::Directory | Display::Tree = tab.display_mode {
            tab.filename_to_clipboard();
        if let Display::Directory | Display::Tree = tab.display_mode {
            tab.filepath_to_clipboard();
    /// Executes a `dragon-drop` command on the selected file.
    /// It obviously requires the `dragon-drop` command to be installed.
    pub fn drag_n_drop(status: &mut Status) -> Result<()> {
        SpecificCommand::drag_n_drop(status);
    /// Set the current selected file as wallpaper with `nitrogen`.
    /// Requires `nitrogen` to be installed.
    pub fn set_wallpaper(tab: &Tab) -> Result<()> {
        SpecificCommand::set_wallpaper(tab);
    /// Display mediainfo details of an image
    pub fn mediainfo(tab: &mut Tab) -> Result<()> {
        SpecificCommand::mediainfo(tab);
    /// Display a diff between the first 2 flagged files or dir.
    pub fn diff(status: &mut Status) -> Result<()> {
        SpecificCommand::diff(status);
        if status.menu.flagged.is_empty() {
        status.menu.trash.update()?;
        for flagged in status.menu.flagged.content.iter() {
            status.menu.trash.trash(flagged)?;
        status.menu.flagged.clear();
        status.current_tab_mut().refresh_view()?;

        status.menu.trash.update()?;
            .current_tab_mut()
        status.menu.trash.update()?;
            .current_tab_mut()
        if status.menu.encrypted_devices.is_empty() {
            status.menu.encrypted_devices.update()?;
            .current_tab_mut()
        status.menu.removable_devices = RemovableDevices::from_gio();
            .current_tab_mut()
        match status
            .internal_settings
            .opener
            .open_single(&path::PathBuf::from(
                shellexpand::tilde(CONFIG_PATH).to_string(),
            )) {
            .current_tab_mut()
    pub fn command(status: &mut Status) -> Result<()> {
        status
            .current_tab_mut()
            .set_edit_mode(Edit::InputCompleted(InputCompleted::Command));
        status.menu.completion.reset();
    /// Execute a custom event on the selected file
    pub fn custom(status: &mut Status, input_string: &str) -> Result<()> {
        status.run_custom_command(input_string)
    pub fn remote_mount(tab: &mut Tab) -> Result<()> {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Remote));
    pub fn click_file(status: &mut Status, row: u16, col: u16) -> Result<()> {
        status.click(row, col)
    }

    pub fn select_pane(status: &mut Status, col: u16) -> Result<()> {
        status.select_tab_from_col(col)
    }

    pub fn click_first_line(col: u16, status: &mut Status, binds: &Bindings) -> Result<()> {
        status.first_line_action(col, binds)
    }

    pub fn lazygit(status: &mut Status) -> Result<()> {
        TuiApplications::open_program(status, LAZYGIT)
    }

    pub fn ncdu(status: &mut Status) -> Result<()> {
        TuiApplications::open_program(status, NCDU)
    }

        let tab = status.current_tab_mut();