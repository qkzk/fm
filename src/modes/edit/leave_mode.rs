use std::borrow::Borrow;
use std::fs;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};

use crate::app::Status;
use crate::common::is_program_in_path;
use crate::common::path_to_string;
use crate::common::string_to_path;
use crate::common::SSHFS_EXECUTABLE;
use crate::config::Bindings;
use crate::event::ActionMap;
use crate::event::EventAction;
use crate::io::execute_and_capture_output_with_path;
use crate::io::execute_custom;
use crate::log_info;
use crate::log_line;
use crate::modes::BlockDeviceAction;
use crate::modes::Display;
use crate::modes::Edit;
use crate::modes::FilterKind;
use crate::modes::InputSimple;
use crate::modes::MarkAction;
use crate::modes::Navigate;
use crate::modes::NodeCreation;
use crate::modes::PasswordUsage;
use crate::modes::SelectableContent;
use crate::modes::SortKind;

use super::InputCompleted;

/// Methods called when executing something with Enter key.
pub struct LeaveMode;

impl LeaveMode {
    pub fn leave_edit_mode(status: &mut Status, binds: &Bindings) -> Result<()> {
        let must_refresh = status.current_tab().edit_mode.must_refresh();
        let must_reset_mode = status.current_tab().edit_mode.must_reset_mode();

        match status.current_tab().edit_mode {
            Edit::Nothing => Ok(()),
            Edit::InputSimple(InputSimple::Rename) => LeaveMode::rename(status),
            Edit::InputSimple(InputSimple::Newfile) => LeaveMode::new_file(status),
            Edit::InputSimple(InputSimple::Newdir) => LeaveMode::new_dir(status),
            Edit::InputSimple(InputSimple::Chmod) => LeaveMode::chmod(status),
            Edit::InputSimple(InputSimple::RegexMatch) => LeaveMode::regex(status),
            Edit::InputSimple(InputSimple::SetNvimAddr) => LeaveMode::set_nvim_addr(status),
            Edit::InputSimple(InputSimple::Shell) => LeaveMode::shell(status),
            Edit::InputSimple(InputSimple::Sort) => LeaveMode::sort(status),
            Edit::InputSimple(InputSimple::Filter) => LeaveMode::filter(status),
            Edit::InputSimple(InputSimple::Password(action, usage)) => {
                LeaveMode::password(status, action, usage)
            }
            Edit::InputSimple(InputSimple::Remote) => LeaveMode::remote(status),
            Edit::Navigate(Navigate::Jump) => LeaveMode::jump(status),
            Edit::Navigate(Navigate::History) => LeaveMode::history(status),
            Edit::Navigate(Navigate::Shortcut) => LeaveMode::shortcut(status),
            Edit::Navigate(Navigate::Trash) => LeaveMode::trash(status),
            Edit::Navigate(Navigate::Bulk) => LeaveMode::bulk(status),
            Edit::Navigate(Navigate::TuiApplication) => LeaveMode::shellmenu(status),
            Edit::Navigate(Navigate::CliApplication) => LeaveMode::cli_info(status),
            Edit::Navigate(Navigate::EncryptedDrive) => Ok(()),
            Edit::Navigate(Navigate::Marks(MarkAction::New)) => LeaveMode::marks_update(status),
            Edit::Navigate(Navigate::Marks(MarkAction::Jump)) => LeaveMode::marks_jump(status),
            Edit::Navigate(Navigate::Compress) => LeaveMode::compress(status),
            Edit::Navigate(Navigate::Context) => LeaveMode::context(status, binds),
            Edit::Navigate(Navigate::RemovableDevices) => Ok(()),
            Edit::InputCompleted(InputCompleted::Exec) => LeaveMode::exec(status),
            Edit::InputCompleted(InputCompleted::Search) => LeaveMode::search(status),
            Edit::InputCompleted(InputCompleted::Goto) => LeaveMode::goto(status),
            Edit::InputCompleted(InputCompleted::Command) => LeaveMode::command(status, binds),
            // To avoid mistakes, the default answer is No. We do nothing here.
            Edit::NeedConfirmation(_) => Ok(()),
        }?;

        status.menu.input.reset();
        if must_reset_mode {
            status.reset_edit_mode()?;
        }
        if must_refresh {
            status.refresh_status()?;
        }
        Ok(())
    }

    /// Restore a file from the trash if possible.
    /// Parent folders are created if needed.
    pub fn trash(status: &mut Status) -> Result<()> {
        status.menu.trash.restore()?;
        status.reset_edit_mode()?;
        status.current_tab_mut().refresh_view()?;
        status.update_second_pane_for_preview()
    }

    /// Jump to the current mark.
    pub fn marks_jump(status: &mut Status) -> Result<()> {
        let marks = status.menu.marks.clone();
        let tab = status.current_tab_mut();
        if let Some((_, path)) = marks.selected() {
            tab.cd(path)?;
            tab.window.reset(tab.directory.content.len());
            status.menu.input.reset();
        }
        status.update_second_pane_for_preview()
    }

    /// Update the selected mark with the current path.
    /// Doesn't change its char.
    /// If it doesn't fail, a new pair will be set with (oldchar, new path).
    pub fn marks_update(status: &mut Status) -> Result<()> {
        let marks = status.menu.marks.clone();
        if let Some((ch, _)) = marks.selected() {
            let len = status.current_tab().directory.content.len();
            let p = status.tabs[status.index].directory.path.borrow();
            status.menu.marks.new_mark(*ch, p)?;
            log_line!("Saved mark {ch} -> {p}", p = p.display());
            status.current_tab_mut().window.reset(len);
            status.menu.input.reset();
        }
        Ok(())
    }

    /// Execute the picked bulk command and reset the menu bulk to None.
    pub fn bulk(status: &mut Status) -> Result<()> {
        status.execute_bulk()?;
        status.menu.bulk = None;
        status.update_second_pane_for_preview()
    }

    /// Execute a shell command picked from the tui_applications menu.
    /// It will be run an a spawned terminal
    pub fn shellmenu(status: &mut Status) -> Result<()> {
        status.menu.tui_applications.execute(status)
    }

    pub fn cli_info(status: &mut Status) -> Result<()> {
        let (output, command) = status.menu.cli_applications.execute(status)?;
        log_info!("command {command}, output\n{output}");
        status.preview_command_output(output, command);
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
        status.internal_settings.nvim_server = status.menu.input.string();
        status.reset_edit_mode()?;
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
    pub fn shell(status: &mut Status) -> Result<()> {
        status.parse_shell_command()?;
        Ok(())
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
                .directory
                .selected()
                .context("rename: couldn't parse selected file")?
                .path
                .borrow()
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
        if status.current_tab().directory.content.is_empty() {
            return Err(anyhow!("exec: empty directory"));
        }
        let exec_command = status.menu.input.string();
        let selected_file = &status.current_tab().current_file_string()?;
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
                let next_index = status.current_tab().directory.index;
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
        let len = status.current_tab().directory.content.len();
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

    fn sort(status: &mut Status) -> Result<()> {
        status.current_tab_mut().settings.sort_kind = match status.current_tab().display_mode {
            Display::Tree => SortKind::tree_default(),
            _ => SortKind::default(),
        };
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
        let index = tab.directory.select_file(&file);
        tab.scroll_to(index);
        log_info!("leave history {path:?} {file:?} {index}");
        status.update_second_pane_for_preview()
    }

    /// Execute a password command (sudo or device passphrase).
    pub fn password(
        status: &mut Status,
        action: Option<BlockDeviceAction>,
        usage: PasswordUsage,
    ) -> Result<()> {
        status.execute_password_command(action, usage)
    }

    /// Compress the flagged files into an archive.
    /// Compression method is chosen by the user.
    /// The archive is created in the current directory and is named "archive.tar.??" or "archive.zip".
    /// Files which are above the CWD are filtered out since they can't be added to an archive.
    /// Archive creation depends on CWD so we ensure it's set to the selected tab.
    pub fn compress(status: &mut Status) -> Result<()> {
        let here = &status.current_tab().directory.path;
        std::env::set_current_dir(here)?;
        let files_with_relative_paths: Vec<std::path::PathBuf> = status
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

    /// Open a menu with most common actions
    pub fn context(status: &mut Status, binds: &Bindings) -> Result<()> {
        let command = status.menu.context.matcher().to_owned();
        EventAction::reset_mode(status)?;
        command.matcher(status, binds)
    }

    /// Execute the selected command.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    pub fn command(status: &mut Status, binds: &Bindings) -> Result<()> {
        let command_str = status.menu.completion.current_proposition();
        let Ok(command) = ActionMap::from_str(command_str) else {
            return Ok(());
        };
        log_info!("Command {command}");

        command.matcher(status, binds)
    }

    /// Apply a filter to the displayed files.
    /// See `crate::filter` for more details.
    pub fn filter(status: &mut Status) -> Result<()> {
        let filter = FilterKind::from_input(&status.menu.input.string());
        status.current_tab_mut().settings.set_filter(filter);
        status.menu.input.reset();
        // ugly hack to please borrow checker :(
        status.tabs[status.index].directory.reset_files(
            &status.tabs[status.index].settings,
            &status.tabs[status.index].users,
        )?;
        if let Display::Tree = status.current_tab().display_mode {
            status.current_tab_mut().make_tree(None)?;
        }
        let len = status.current_tab().directory.content.len();
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
