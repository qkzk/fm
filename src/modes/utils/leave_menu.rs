use std::str::FromStr;

use anyhow::{bail, Context, Result};

use crate::app::Status;
use crate::common::{path_to_string, rename_fullpath, string_to_path};
use crate::config::Bindings;
use crate::event::{ActionMap, EventAction, FmEvents};
use crate::modes::{
    Content, InputCompleted, InputSimple, Leave, MarkAction, Menu, MountAction, Navigate,
    NodeCreation, PasswordUsage, PickerCaller, TerminalApplications,
};
use crate::{log_info, log_line};

/// Methods called when executing something with Enter key.
pub struct LeaveMenu;

impl LeaveMenu {
    pub fn leave_menu(status: &mut Status, binds: &Bindings) -> Result<()> {
        status
            .menu
            .input_history
            .update(status.current_tab().menu_mode, &status.menu.input.string())?;
        let must_refresh = status.current_tab().menu_mode.must_refresh();
        let must_reset_mode = status.current_tab().menu_mode.must_reset_mode();

        match status.current_tab().menu_mode {
            Menu::Nothing => Ok(()),
            Menu::InputSimple(InputSimple::Rename) => LeaveMenu::rename(status),
            Menu::InputSimple(InputSimple::Newfile) => LeaveMenu::new_file(status),
            Menu::InputSimple(InputSimple::Newdir) => LeaveMenu::new_dir(status),
            Menu::InputSimple(InputSimple::Chmod) => LeaveMenu::chmod(status),
            Menu::InputSimple(InputSimple::RegexMatch) => LeaveMenu::regex_match(status),
            Menu::InputSimple(InputSimple::SetNvimAddr) => LeaveMenu::set_nvim_addr(status),
            Menu::InputSimple(InputSimple::ShellCommand) => LeaveMenu::shell_command(status),
            Menu::InputSimple(InputSimple::Sort) => LeaveMenu::sort(status),
            Menu::InputSimple(InputSimple::Filter) => LeaveMenu::filter(status),
            Menu::InputSimple(InputSimple::Password(action, usage)) => {
                LeaveMenu::password(status, action, usage)
            }
            Menu::InputSimple(InputSimple::CloudNewdir) => {
                LeaveMenu::cloud_newdir(status)?;
                return Ok(());
            }
            Menu::InputSimple(InputSimple::Remote) => LeaveMenu::remote(status),
            Menu::Navigate(Navigate::History) => LeaveMenu::history(status),
            Menu::Navigate(Navigate::Shortcut) => LeaveMenu::shortcut(status),
            Menu::Navigate(Navigate::Trash) => LeaveMenu::trash(status),
            Menu::Navigate(Navigate::TuiApplication) => LeaveMenu::tui_application(status),
            Menu::Navigate(Navigate::CliApplication) => LeaveMenu::cli_info(status),
            Menu::Navigate(Navigate::Cloud) => {
                LeaveMenu::cloud_enter(status)?;
                return Ok(());
            }
            Menu::Navigate(Navigate::Marks(MarkAction::New)) => LeaveMenu::marks_update(status),
            Menu::Navigate(Navigate::Marks(MarkAction::Jump)) => LeaveMenu::marks_jump(status),
            Menu::Navigate(Navigate::TempMarks(MarkAction::New)) => {
                LeaveMenu::tempmark_update(status)
            }
            Menu::Navigate(Navigate::TempMarks(MarkAction::Jump)) => LeaveMenu::tempmark_jp(status),
            Menu::Navigate(Navigate::Compress) => LeaveMenu::compress(status),
            Menu::Navigate(Navigate::Mount) => LeaveMenu::go_to_mount(status),
            Menu::Navigate(Navigate::Context) => LeaveMenu::context(status, binds),
            Menu::Navigate(Navigate::Picker) => {
                LeaveMenu::picker(status)?;
                return Ok(());
            }
            Menu::Navigate(Navigate::Flagged) => LeaveMenu::flagged(status),
            Menu::InputCompleted(InputCompleted::Exec) => {
                LeaveMenu::exec(status)?;
                return Ok(());
            }
            Menu::InputCompleted(InputCompleted::Search) => Ok(()),
            Menu::InputCompleted(InputCompleted::Cd) => LeaveMenu::cd(status),
            Menu::InputCompleted(InputCompleted::Action) => LeaveMenu::action(status),
            // To avoid mistakes, the default answer is No. We do nothing here.
            Menu::NeedConfirmation(_) => Ok(()),
        }?;

        status.menu.input.reset();
        if must_reset_mode {
            status.reset_menu_mode()?;
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
        status.reset_menu_mode()?;
        status.current_tab_mut().refresh_view()?;
        status.update_second_pane_for_preview()
    }

    /// Jump to the current mark.
    fn marks_jump(status: &mut Status) -> Result<()> {
        if let Some((_, path)) = &status.menu.marks.selected() {
            let len = status.current_tab().directory.content.len();
            status.tabs[status.index].cd(path)?;
            status.current_tab_mut().window.reset(len);
            status.menu.input.reset();
        }
        status.update_second_pane_for_preview()
    }

    /// Update the selected mark with the current path.
    /// Doesn't change its char.
    /// If it doesn't fail, a new pair will be set with (oldchar, new path).
    fn marks_update(status: &mut Status) -> Result<()> {
        if let Some((ch, _)) = status.menu.marks.selected() {
            let len = status.current_tab().directory.content.len();
            let new_path = &status.tabs[status.index].directory.path;
            log_line!("Saved mark {ch} -> {p}", p = new_path.display());
            status.menu.marks.new_mark(*ch, new_path)?;
            status.current_tab_mut().window.reset(len);
            status.menu.input.reset();
        }
        Ok(())
    }

    /// Update the selected mark with the current path.
    /// Doesn't change its char.
    /// If it doesn't fail, a new pair will be set with (oldchar, new path).
    fn tempmark_update(status: &mut Status) -> Result<()> {
        let index = status.menu.temp_marks.index;
        let len = status.current_tab().directory.content.len();
        let new_path = &status.tabs[status.index].directory_of_selected()?;
        status
            .menu
            .temp_marks
            .set_mark(index as _, new_path.to_path_buf());
        log_line!("Saved temp mark {index} -> {p}", p = new_path.display());
        status.current_tab_mut().window.reset(len);
        status.menu.input.reset();
        Ok(())
    }

    /// Jump to the current mark.
    fn tempmark_jp(status: &mut Status) -> Result<()> {
        let Some(opt_path) = &status.menu.temp_marks.selected() else {
            log_info!("no selected temp mark");
            return Ok(());
        };
        let Some(path) = opt_path else {
            return Ok(());
        };
        let len = status.current_tab().directory.content.len();
        status.tabs[status.index].cd(path)?;
        status.current_tab_mut().window.reset(len);
        status.menu.input.reset();

        status.update_second_pane_for_preview()
    }

    /// Execute a shell command picked from the tui_applications menu.
    /// It will be run an a spawned terminal
    fn tui_application(status: &mut Status) -> Result<()> {
        status.internal_settings.disable_display();
        status.menu.tui_applications.execute(status)?;
        status.internal_settings.enable_display();
        Ok(())
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
        status.cloud_create_newdir(status.menu.input.string())?;
        status.reset_menu_mode()?;
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
        status.reset_menu_mode()?;
        Ok(())
    }

    /// Select the first file matching the typed regex in current dir.
    fn regex_match(status: &mut Status) -> Result<()> {
        status.flag_from_regex()?;
        status.menu.input.reset();
        Ok(())
    }

    /// Execute a shell command typed by the user.
    /// but expansions are supported
    /// It won't return an `Err` if the command fail but log a message.
    fn shell_command(status: &mut Status) -> Result<()> {
        status.execute_shell_command_from_input()?;
        Ok(())
    }

    /// Execute a rename of the selected file.
    /// It uses the `fs::rename` function and has the same limitations.
    /// Intermediates directory are created if needed.
    /// It acts like a move (without any confirmation...)
    /// The new file is selected.
    fn rename(status: &mut Status) -> Result<()> {
        if status.menu.input.is_empty() {
            log_line!("Can't rename: new name is empty");
            log_info!("Can't rename: new name is empty");
            return Ok(());
        }
        let new_path = status.menu.input.string();
        let old_path = status.current_tab().current_file()?.path;
        match rename_fullpath(&old_path, &new_path) {
            Ok(()) => {
                status.rename_marks(&old_path, &new_path)?;
                status.current_tab_mut().refresh_view()?;
                status.current_tab_mut().cd_to_file(&new_path)?;
            }
            Err(error) => {
                log_info!(
                    "Error renaming {old_path} to {new_path}. Error: {error}",
                    old_path = old_path.display()
                );
                log_line!(
                    "Error renaming {old_path} to {new_path}. Error: {error}",
                    old_path = old_path.display()
                );
            }
        }
        Ok(())
    }

    /// Creates a new file with input string as name.
    /// Nothing is done if the file already exists.
    fn new_file(status: &mut Status) -> Result<()> {
        match NodeCreation::Newfile.create(status) {
            Ok(path) => {
                status.current_tab_mut().go_to_file(&path);
                status.menu.flagged.push(path);
                status.refresh_tabs()?;
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
                status.refresh_tabs()?;
                status.current_tab_mut().go_to_file(&path);
                status.menu.flagged.push(path);
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
            bail!("exec: empty directory")
        }
        let exec_command = status.menu.input.string();
        if status.execute_shell_command(
            exec_command,
            Some(status.menu.flagged.as_strings()),
            false,
        )? {
            status.menu.completion.reset();
            status.menu.input.reset();
        }
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
        status.thumbnail_queue_clear();
        status.menu.input.reset();
        status.current_tab_mut().cd_to_file(&path)?;
        let len = status.current_tab().directory.content.len();
        status.current_tab_mut().window.reset(len);
        status.update_second_pane_for_preview()
    }

    /// Move to the selected shortcut.
    /// It may fail if the user has no permission to visit the path.
    fn shortcut(status: &mut Status) -> Result<()> {
        let path = status
            .menu
            .shortcut
            .selected()
            .context("exec shortcut: empty shortcuts")?;
        status.tabs[status.index].cd(path)?;
        status.current_tab_mut().refresh_view()?;
        status.update_second_pane_for_preview()
    }

    fn sort(status: &mut Status) -> Result<()> {
        status.current_tab_mut().set_sortkind_per_mode();
        status.update_second_pane_for_preview()?;
        status.focus = status.focus.to_parent();
        Ok(())
    }

    /// Move back to a previously visited path.
    /// It may fail if the user has no permission to visit the path
    fn history(status: &mut Status) -> Result<()> {
        status.current_tab_mut().history_cd_to_last()?;
        status.update_second_pane_for_preview()
    }

    /// Execute a password command (sudo or device passphrase).
    fn password(
        status: &mut Status,
        action: Option<MountAction>,
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
        status.compress()
    }

    /// Open a menu with most common actions
    fn context(status: &mut Status, binds: &Bindings) -> Result<()> {
        let command = status.menu.context.matcher().to_owned();
        EventAction::reset_mode(status)?;
        command.matcher(status, binds)
    }

    /// Execute the selected action.
    /// Some commands does nothing as they require to be executed from a specific
    /// context.
    fn action(status: &mut Status) -> Result<()> {
        let action_str = status.menu.completion.current_proposition();
        let Ok(action) = ActionMap::from_str(action_str) else {
            return Ok(());
        };

        status.reset_menu_mode()?;
        status.focus = status.focus.to_parent();
        status.fm_sender.send(FmEvents::Action(action))?;
        Ok(())
    }

    /// Apply a filter to the displayed files.
    /// See `crate::filter` for more details.
    fn filter(status: &mut Status) -> Result<()> {
        status.filter()?;
        status.menu.input.reset();
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
        match status.current_tab().menu_mode {
            Menu::Navigate(Navigate::Mount) => status.go_to_normal_drive(),
            _ => Ok(()),
        }
    }

    fn picker(status: &mut Status) -> Result<()> {
        let Some(caller) = &status.menu.picker.caller else {
            return Ok(());
        };
        match caller {
            PickerCaller::Cloud => status.cloud_load_config(),
            PickerCaller::Menu(menu) => EventAction::reenter_menu_from_picker(status, *menu),
            PickerCaller::Unknown => Ok(()),
        }
    }

    fn flagged(status: &mut Status) -> Result<()> {
        status.jump_flagged()
    }
}
