use crate::constant_strings_paths::DEFAULT_DRAGNDROP;
/// Every kind of mutation of the application is defined here.
/// It mutates `Status` or its child `tab`.
    /// Reset the selected tab view to the default.
    /// When a rezise event occurs, we may hide the second panel if the width
    /// isn't sufficiant to display enough information.
    /// We also need to know the new height of the terminal to start scrolling
    /// up or down.
    /// Remove every flag on files in this directory and others.
    /// Flag all files in the current directory.
        status.reset_tabs_view()
    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
        status.reset_tabs_view()
    /// Toggle a single flag and move down one row.
    /// Move to the next file in the jump list.
    /// Move to the previous file in the jump list.
    /// Change to CHMOD mode allowing to edit permissions of a file.
        status.reset_tabs_view()
    /// Enter JUMP mode, allowing to jump to any flagged file.
    /// Does nothing if no file is flagged.
    /// Enter Marks new mode, allowing to bind a char to a path.
    /// Enter Marks jump mode, allowing to jump to a marked file.
    /// Execute a new mark, saving it to a config file for futher use.
    /// Execute a jump to a mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    /// Enter bulkrename mode, opening a random temp file where the user
    /// can edit the selected filenames.
    /// Once the temp file is saved, those file names are changed.
        Bulkrename::new(status.filtered_flagged_files())?.rename(&status.opener)?;
    /// Copy the flagged file to current directory.
    /// A progress bar is displayed and a notification is sent once it's done.
    /// Move the flagged file to current directory.
    /// A progress bar is displayed and a notification is sent once it's done.
    /// Recursively delete all flagged files.
    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
        if status.selected().input.is_empty() {
            u32::from_str_radix(&status.selected().input.string(), 8).unwrap_or(0_u32);
        status.reset_tabs_view()

    fn _exec_last_edition(status: &mut Status) -> FmResult<()> {
    /// Execute a jump to the selected flagged file.
    /// If the user selected a directory, we jump inside it.
    /// Otherwise, we jump to the parent and select the file.
        status.selected().input.clear();
    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    /// Select the first file matching the typed regex in current dir.
    /// Leave current mode to normal mode.
    /// Reset the inputs and completion, reset the window, exit the preview.
    /// Move up one row if possible.
    /// Move down one row if possible.
    /// Move to the top of the current directory.
    /// Move up 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves up one page.
    /// Move to the bottom of current view.
    /// Move the cursor to the start of line.
    /// Move the cursor to the end of line.
    /// Move down 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves down one page.
    /// Select a given row, if there's something in it.
    /// Select the next shortcut.
    /// Select the previous shortcut.
    /// Select the next element in history of visited files.
    /// Select the previous element in history of visited files.
    /// Move to parent directory if there's one.
    /// Raise an FmError in root folder.
    /// Add the starting directory to history.
    /// Move the cursor left one block.
    /// Open the file with configured opener or enter the directory.
    pub fn exec_file(status: &mut Status) -> FmResult<()> {
        let tab = status.selected();
            Self::event_open_file(status)
    /// Move the cursor to the right in input string.
    /// Delete the char to the left in input string.
    /// Delete all chars right of the cursor in input string.
    /// Add a char to input string, look for a possible completion.
    /// Enter a copy paste mode.
    /// Enter the 'move' mode.
    /// Enter the new dir mode.
    /// Enter the new file mode.
    /// Enter the execute mode. Most commands must be executed to allow for
    /// a confirmation.
    /// Enter preview mode.
    /// Every file can be previewed. See the `crate::enum::Preview` for
    /// more details on previewing.
    /// Enter the delete mode.
    /// A confirmation is then asked.
    /// Display the help which can be navigated and displays the configrable
    /// binds.
    pub fn event_help(status: &mut Status) -> FmResult<()> {
        let help = status.help.clone();
        let tab = status.selected();
        tab.preview = Preview::help(help);
    /// Enter the search mode.
    /// Matching items are displayed as you type them.
        tab.searched = None;
    /// Enter the regex mode.
    /// Every file matching the typed regex will be flagged.
    /// Enter the sort mode, allowing the user to select a sort method.
    /// Once a quit event is received, we change a flag and break the main loop.
    /// It's usefull to reset the cursor before leaving the application.
    /// Reset the mode to normal.
    /// Sort the file with given criteria
    /// Valid kind of sorts are :
    /// by kind : directory first, files next, in alphanumeric order
    /// by filename,
    /// by date of modification,
    /// by size,
    /// by extension.
    /// The first letter is used to identify the method.
    /// If the user types an uppercase char, the sort is reverse.
            'k' | 'K' => tab.path_content.sort_by = SortBy::Kind,
            'n' | 'N' => tab.path_content.sort_by = SortBy::Filename,
            'm' | 'M' => tab.path_content.sort_by = SortBy::Date,
            's' | 'S' => tab.path_content.sort_by = SortBy::Size,
            'e' | 'E' => tab.path_content.sort_by = SortBy::Extension,
        if c.is_uppercase() {
            tab.path_content.reverse = true
        }
    /// Insert a char in the input string.
    /// Toggle the display of hidden files.
    /// Open a file with custom opener.
    pub fn event_open_file(status: &mut Status) -> FmResult<()> {
        match status.opener.open(
            status
                .selected_non_mut()
                .path_content
                status.selected_non_mut().path_content.selected_file(),
    /// Enter the rename mode.
    /// Enter the goto mode where an user can type a path to jump to.
    /// Open a new terminal in current directory.
    /// The shell is a fork of current process and will exit if the application
    /// is terminated first.
    pub fn event_shell(status: &mut Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
            &status.opener.terminal.clone(),
    /// Enter the history mode, allowing to navigate to previously visited
    /// directory.
    /// Enter the shortcut mode, allowing to visite predefined shortcuts.
    /// Basic folders (/, /dev... $HOME) and mount points (even impossible to
    /// visit ones) are proposed.
    /// A right click opens a file or a directory.
                Self::exec_file(status)
                Self::event_open_file(status)
    /// Replace the input string by the selected completion.
    /// Send a signal to parent NVIM process, picking the selected file.
    /// If no RPC server were provided at launch time - which may happen for
    /// reasons unknow to me - it does nothing.
    /// It requires the "nvim-send" application to be in $PATH.
    /// Copy the selected filename to the clipboard. Only the filename.
    /// Copy the selected filepath to the clipboard. The absolute path.
    /// Enter the filter mode, where you can filter.
    /// See `crate::filter::Filter` for more details.
    /// Move back in history to the last visited directory.
    /// Move to $HOME aka ~.
    fn nvim_listen_address(tab: &Tab) -> Result<String, std::env::VarError> {
    /// Execute a rename of the selected file.
    /// It uses the `fs::rename` function and has the same limitations.
    /// We only tries to rename in the same directory, so it shouldn't be a problem.
    /// Filename is sanitized before processing.
            tab.path_content
                .path
                .to_path_buf()
                .join(&sanitize_filename::sanitize(tab.input.string())),
    /// Creates a new file with input string as name.
    /// We use `fs::File::create` internally, so if the file already exists,
    /// it will be overwritten.
    /// Filename is sanitized before processing.
        fs::File::create(
            tab.path_content
                .path
                .join(sanitize_filename::sanitize(tab.input.string())),
        )?;
    /// Creates a new directory with input string as name.
    /// We use `fs::create_dir` internally so it will fail if the input string
    /// is not an end point in the file system.
    /// ie. the user can create `newdir` but not `newdir/newfolder`.
    /// It will also fail if the directory already exists.
    /// Directory name is sanitized before processing.
        match fs::create_dir(
            tab.path_content
                .path
                .join(sanitize_filename::sanitize(tab.input.string())),
        ) {
    /// Tries to execute the selected file with an executable which is read
    /// from the input string. It will fail silently if the executable can't
    /// be found.
    /// Optional parameters can be passed normally. ie. `"ls -lah"`
        let exec_command = tab.input.string();
                    &format!("can't find command {}", command),
    /// Executes a `dragon-drop` command on the selected file.
    /// It obviously requires the `dragon-drop` command to be installed.
            DEFAULT_DRAGNDROP,
                    "can't find dragon-drop in the system. Is the application installed?",
    /// Executes a search in current folder, selecting the first file matching
    /// the current completion proposition.
    /// ie. If you typed `"jpg"` before, it will move to the first file
    /// whose filename contains `"jpg"`.
    /// The current order of files is used.
        let searched = tab.input.string();
        if searched.is_empty() {
            tab.searched = None;
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
            if file.filename.contains(&searched_name) {
                found = true;
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
    /// Move to the folder typed by the user.
    /// The first completion proposition is used, `~` expansion is done.
    /// If no result were found, no cd is done and we go back to normal mode
    /// silently.
        if tab.completion.is_empty() {
            return Ok(());
        }
        let completed = tab.completion.current_proposition();
        let path = string_to_path(completed)?;
    /// Move to the selected shortcut.
    /// It may fail if the user has no permission to visit the path.
    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    /// Apply a filter to the displayed files.
    /// See `crate::filter` for more details.
        let filter = FilterKind::from_input(&tab.input.string());
    /// Move up one row in modes allowing movement.
    /// Does nothing if the selected item is already the first in list.
    /// Move down one row in modes allowing movements.
    /// Does nothing if the user is already at the bottom.
    /// Move to parent in normal mode,
    /// move left one char in mode requiring text input.
    /// Move to child if any or open a regular file in normal mode.
    /// Move the cursor one char to right in mode requiring text input.
            Mode::Normal => EventExec::exec_file(status),
    /// Delete a char to the left in modes allowing edition.
    /// Delete all chars to the right in mode allowing edition.
    /// Move to leftmost char in mode allowing edition.
    /// Move to the bottom in any mode.
    /// Move up 10 lines in normal mode and preview.
    /// Move down 10 lines in normal & preview mode.
    /// Execute the mode.
    /// In modes requiring confirmation or text input, it will execute the
    /// related action.
    /// In normal mode, it will open the file.
    /// Reset to normal mode afterwards.
            Mode::Normal => EventExec::exec_file(status)?,
    /// Change tab in normal mode with dual pane displayed,
    /// insert a completion in modes allowing completion.
    /// Change tab in normal mode.
    /// Start a fuzzy find with skim.
    /// ATM idk how to avoid using the whole screen.
        status.fill_tabs_with_skim()
    /// Copy the filename of the selected file in normal mode.
    /// Copy the filepath of the selected file in normal mode.
    /// Refresh the current view, reloading the files. Move the selection to top.
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
    /// Toggle between dualpane and single pane. Does nothing if the width
    /// is too low to display both panes.
    pub fn event_toggle_dualpane(status: &mut Status) -> FmResult<()> {
        status.dual_pane = !status.dual_pane;
        status.select_tab(0)?;
        Ok(())
    }


fn string_to_path(path_string: String) -> FmResult<path::PathBuf> {
    let expanded_cow_path = shellexpand::tilde(&path_string);
    let expanded_target: &str = expanded_cow_path.borrow();
    Ok(std::fs::canonicalize(expanded_target)?)
}