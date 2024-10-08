use std::borrow::Borrow;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};

use crate::app::Status;
use crate::common::{path_to_string, rename, string_to_path};
use crate::config::Bindings;
use crate::event::{ActionMap, EventAction};
use crate::io::execute_custom;
use crate::modes::{
    BlockDeviceAction, CLApplications, Content, Display, Edit, InputCompleted, InputSimple, Leave,
    MarkAction, Navigate, NodeCreation, PasswordUsage, PickerCaller, Search, SortKind,
};
use crate::{log_info, log_line};

/// Methods called when executing something with Enter key.
pub struct LeaveMode;

impl LeaveMode {
    pub fn leave_edit_mode(status: &mut Status, binds: &Bindings) -> Result<()> {
        status
            .menu
            .input_history
            .update(status.current_tab().edit_mode, &status.menu.input.string())?;
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
            Edit::InputSimple(InputSimple::CloudNewdir) => {
                LeaveMode::cloud_newdir(status)?;
                return Ok(());
            }
            Edit::InputSimple(InputSimple::Remote) => LeaveMode::remote(status),
            Edit::Navigate(Navigate::History) => LeaveMode::history(status),
            Edit::Navigate(Navigate::Shortcut) => LeaveMode::shortcut(status),
            Edit::Navigate(Navigate::Trash) => LeaveMode::trash(status),
            Edit::Navigate(Navigate::TuiApplication) => LeaveMode::shellmenu(status),
            Edit::Navigate(Navigate::CliApplication) => LeaveMode::cli_info(status),
            Edit::Navigate(Navigate::Cloud) => {
                LeaveMode::cloud_enter(status)?;
                return Ok(());
            }
            Edit::Navigate(Navigate::EncryptedDrive) => LeaveMode::go_to_mount(status),
            Edit::Navigate(Navigate::Marks(MarkAction::New)) => LeaveMode::marks_update(status),
            Edit::Navigate(Navigate::Marks(MarkAction::Jump)) => LeaveMode::marks_jump(status),
            Edit::Navigate(Navigate::Compress) => LeaveMode::compress(status),
            Edit::Navigate(Navigate::Context) => LeaveMode::context(status, binds),
            Edit::Navigate(Navigate::RemovableDevices) => LeaveMode::go_to_mount(status),
            Edit::Navigate(Navigate::Picker) => {
                LeaveMode::picker(status)?;
                return Ok(());
            }
            Edit::Navigate(Navigate::Flagged) => LeaveMode::flagged(status),
            Edit::InputCompleted(InputCompleted::Exec) => LeaveMode::exec(status),
            Edit::InputCompleted(InputCompleted::Search) => LeaveMode::search(status, true),
            Edit::InputCompleted(InputCompleted::Cd) => LeaveMode::cd(status),
            Edit::InputCompleted(InputCompleted::Action) => LeaveMode::action(status, binds),
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
        if status.focus.is_file() {
            return Ok(());
        }
        let _ = status.menu.trash.restore();
        status.reset_edit_mode()?;
        status.current_tab_mut().refresh_view()?;
        status.update_second_pane_for_preview()
    }

    /// Jump to the current mark.
    fn marks_jump(status: &mut Status) -> Result<()> {
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
    fn marks_update(status: &mut Status) -> Result<()> {
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

    /// Execute a shell command picked from the tui_applications menu.
    /// It will be run an a spawned terminal
    fn shellmenu(status: &mut Status) -> Result<()> {
        status.menu.tui_applications.execute(status)
    }

    fn cli_info(status: &mut Status) -> Result<()> {
        let (output, command) = status.menu.cli_applications.execute(status)?;
        log_info!("cli info: command {command}, output\n{output}");
        status.preview_command_output(output, command);
        Ok(())
    }

    fn cloud_enter(status: &mut Status) -> Result<()> {
        status.cloud_enter_file_or_dir()
    }

    fn cloud_newdir(status: &mut Status) -> Result<()> {
        let dirname = status.menu.input.string();
        status.menu.input.reset();
        status.cloud_create_newdir(dirname)?;
        status.reset_edit_mode()?;
        status.cloud_open()
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    fn chmod(status: &mut Status) -> Result<()> {
        status.chmod()
    }

    fn set_nvim_addr(status: &mut Status) -> Result<()> {
        status.internal_settings.nvim_server = status.menu.input.string();
        status.reset_edit_mode()?;
        Ok(())
    }

    /// Select the first file matching the typed regex in current dir.
    fn regex(status: &mut Status) -> Result<()> {
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
    fn shell(status: &mut Status) -> Result<()> {
        status.parse_shell_command()?;
        Ok(())
    }

    /// Execute a rename of the selected file.
    /// It uses the `fs::rename` function and has the same limitations.
    /// Intermediates directory are created if needed.
    /// It acts like a move (without any confirmation...)

    fn rename(status: &mut Status) -> Result<()> {
        let old_path = status.current_tab().current_file()?.path;
        let new_name = status.menu.input.string();
        if let Err(error) = rename(&old_path, &new_name) {
            log_info!(
                "Error renaming {old_path} to {new_name}. Error: {error}",
                old_path = old_path.display()
            );
            return Err(error);
        };
        status.current_tab_mut().refresh_view()
    }

    /// Creates a new file with input string as name.
    /// Nothing is done if the file already exists.
    fn new_file(status: &mut Status) -> Result<()> {
        match NodeCreation::Newfile.create(status) {
            Ok(path) => {
                status.menu.flagged.push(path.clone());
                status.refresh_tabs()?;
                status.current_tab_mut().go_to_file(path);
            }
            Err(error) => log_info!("Error creating file. Error: {error}",),
        }
        Ok(())
    }

    /// Creates a new directory with input string as name.
    /// Nothing is done if the directory already exists.
    /// We use `fs::create_dir` internally so it will fail if the input string
    /// ie. the user can create `newdir` or `newdir/newfolder`.
    fn new_dir(status: &mut Status) -> Result<()> {
        match NodeCreation::Newdir.create(status) {
            Ok(path) => {
                status.menu.flagged.push(path.clone());
                status.refresh_tabs()?;
                status.current_tab_mut().go_to_file(path);
            }
            Err(error) => log_info!("Error creating directory. Error: {error}",),
        }
        Ok(())
    }

    /// Tries to execute the selected file with an executable which is read
    /// from the input string. It will fail silently if the executable can't
    /// be found.
    /// Optional parameters can be passed normally. ie. `"ls -lah"`
    fn exec(status: &mut Status) -> Result<()> {
        if status.current_tab().directory.content.is_empty() {
            return Err(anyhow!("exec: empty directory"));
        }
        // status.menu.completion_tab();
        let exec_command = status.menu.input.string();
        let paths = status.menu.flagged.content();
        if let Ok(success) = execute_custom(exec_command, paths) {
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
    pub fn search(status: &mut Status, should_reset_input: bool) -> Result<()> {
        let searched = &status.menu.input.string();
        if searched.is_empty() {
            status.current_tab_mut().search = Search::empty();
            return Ok(());
        }
        let Ok(mut search) = Search::new(searched) else {
            status.current_tab_mut().search = Search::empty();
            return Ok(());
        };
        if should_reset_input {
            status.menu.input.reset();
        }
        search.execute_search(status)?;
        status.current_tab_mut().search = search;
        Ok(())
    }

    /// Move to the folder typed by the user.
    /// The first completion proposition is used, `~` expansion is done.
    /// If no result were found, no cd is done and we go back to normal mode
    /// silently.
    fn cd(status: &mut Status) -> Result<()> {
        if status.menu.completion.is_empty() {
            return Ok(());
        }
        let completed = status.menu.completion.current_proposition();
        let path = string_to_path(completed)?;
        status.menu.input.reset();
        status.current_tab_mut().cd_to_file(&path)?;
        let len = status.current_tab().directory.content.len();
        status.current_tab_mut().window.reset(len);
        status.update_second_pane_for_preview()
    }

    /// Move to the selected shortcut.
    /// It may fail if the user has no permission to visit the path.
    fn shortcut(status: &mut Status) -> Result<()> {
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
        status.update_second_pane_for_preview()?;
        status.focus = status.focus.to_parent();
        Ok(())
    }

    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    fn history(status: &mut Status) -> Result<()> {
        let Some(file) = status.tabs[status.index].history.selected() else {
            return Ok(());
        };
        let file = file.to_owned();
        status.tabs[status.index].cd_to_file(&file)?;
        status.tabs[status.index].history.drop_queue();
        status.update_second_pane_for_preview()
    }

    /// Execute a password command (sudo or device passphrase).
    fn password(
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
    fn compress(status: &mut Status) -> Result<()> {
        let here = &status.current_tab().directory.path;
        std::env::set_current_dir(here)?;
        let files_with_relative_paths: Vec<std::path::PathBuf> = status
            .flagged_or_selected()
            .iter()
            .filter_map(|abs_path| pathdiff::diff_paths(abs_path, here))
            .filter(|f| !f.starts_with(".."))
            .collect();
        if files_with_relative_paths.is_empty() {
            return Ok(());
        }
        match status
            .menu
            .compression
            .compress(files_with_relative_paths, here)
        {
            Ok(()) => (),
            Err(error) => log_info!("Error compressing files. Error: {error}"),
        }
        Ok(())
    }

    /// Open a menu with most common actions
    fn context(status: &mut Status, binds: &Bindings) -> Result<()> {
        let command = status.menu.context.matcher().to_owned();
        EventAction::reset_mode(status)?;
        command.matcher(status, binds)
    }

    /// Execute the selected command.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    fn action(status: &mut Status, binds: &Bindings) -> Result<()> {
        let command_str = status.menu.completion.current_proposition();
        let Ok(command) = ActionMap::from_str(command_str) else {
            return Ok(());
        };
        log_info!("Command {command}");

        command.matcher(status, binds)
    }

    /// Apply a filter to the displayed files.
    /// See `crate::filter` for more details.
    fn filter(status: &mut Status) -> Result<()> {
        status.set_filter()?;
        status.menu.input.reset();
        let mut search = status.tabs[status.index].search.clone();
        search.reset_paths();
        search.execute_search(status)?;
        status.tabs[status.index].search = search;
        Ok(())
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    fn remote(status: &mut Status) -> Result<()> {
        let current_path = &path_to_string(&status.current_tab().directory_of_selected()?);
        status.menu.mount_remote(current_path);
        Ok(())
    }

    /// Go to the _mounted_ device. Does nothing if the device isn't mounted.
    fn go_to_mount(status: &mut Status) -> Result<()> {
        match status.current_tab().edit_mode {
            Edit::Navigate(Navigate::EncryptedDrive) => status.go_to_encrypted_drive(),
            Edit::Navigate(Navigate::RemovableDevices) => status.go_to_removable(),
            _ => Ok(()),
        }
    }

    fn picker(status: &mut Status) -> Result<()> {
        let Some(caller) = &status.menu.picker.caller else {
            return Ok(());
        };
        match caller {
            PickerCaller::Cloud => status.cloud_load_config(),
            PickerCaller::Unknown => Ok(()),
        }
    }

    fn flagged(status: &mut Status) -> Result<()> {
        status.jump_flagged()
    }
}
