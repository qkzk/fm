use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use skim::SkimItem;
use sysinfo::{Disk, RefreshKind, System, SystemExt};
use tuikit::prelude::{from_keyname, Event};
use tuikit::term::Term;

use crate::app::ClickableLine;
use crate::app::DisplaySettings;
use crate::app::Footer;
use crate::app::Header;
use crate::app::InternalSettings;
use crate::app::Tab;
use crate::common::{args_is_empty, is_sudo_command, path_to_string};
use crate::common::{current_username, disk_space, filename_from_path, is_program_in_path};
use crate::config::Bindings;
use crate::config::Settings;
use crate::io::Args;
use crate::io::Internal;
use crate::io::Kind;
use crate::io::Opener;
use crate::io::MIN_WIDTH_FOR_DUAL_PANE;
use crate::io::{
    execute_and_capture_output_without_check, execute_sudo_command_with_password,
    execute_without_output_with_path, reset_sudo_faillock,
};
use crate::modes::CopyMove;
use crate::modes::Display;
use crate::modes::Edit;
use crate::modes::FileKind;
use crate::modes::InputSimple;
use crate::modes::IsoDevice;
use crate::modes::Menu;
use crate::modes::MountCommands;
use crate::modes::MountRepr;
use crate::modes::NeedConfirmation;
use crate::modes::PasswordKind;
use crate::modes::PasswordUsage;
use crate::modes::Permissions;
use crate::modes::Preview;
use crate::modes::SelectableContent;
use crate::modes::ShellCommandParser;
use crate::modes::Skimer;
use crate::modes::Tree;
use crate::modes::Users;
use crate::modes::{copy_move, regex_matcher};
use crate::modes::{BlockDeviceAction, Navigate};
use crate::{log_info, log_line};

pub enum Window {
    Header,
    Files,
    Menu,
    Footer,
}

/// Holds every mutable parameter of the application itself, except for
/// the "display" information.
/// It holds 2 tabs (left & right), even if only one can be displayed sometimes.
/// It knows which tab is selected, which files are flagged,
/// which jump target is selected, a cache of normal file colors,
/// if we have to display one or two tabs and if all details are shown or only
/// the filename.
/// Mutation of this struct are mostly done externally, by the event crate :
/// `crate::event_exec`.
pub struct Status {
    /// Vector of `Tab`, each of them are displayed in a separate tab.
    pub tabs: [Tab; 2],
    /// Index of the current selected tab
    pub index: usize,

    skimer: Option<Result<Skimer>>,

    /// Navigable menu
    pub menu: Menu,
    /// Display settings
    pub display_settings: DisplaySettings,
    /// Interna settings
    pub internal_settings: InternalSettings,
}

impl Status {
    /// Creates a new status for the application.
    /// It requires most of the information (arguments, configuration, height
    /// of the terminal, the formated help string).
    pub fn new(
        height: usize,
        term: Arc<Term>,
        opener: Opener,
        settings: &Settings,
    ) -> Result<Self> {
        let skimer = None;
        let index = 0;

        let args = Args::parse();
        let path = std::fs::canonicalize(Path::new(&args.path))?;
        let start_dir = if path.is_dir() {
            &path
        } else {
            path.parent().context("")?
        };
        let sys = System::new_with_specifics(RefreshKind::new().with_disks());
        let display_settings = DisplaySettings::new(&args, settings, term.term_size()?.0);
        let mut internal_settings = InternalSettings::new(opener, term, sys);
        let mount_points = internal_settings.mount_points();
        let menu = Menu::new(start_dir, &mount_points)?;

        let users_left = Users::new();
        let users_right = users_left.clone();

        let tabs = [
            Tab::new(&args, height, users_left, settings)?,
            Tab::new(&args, height, users_right, settings)?,
        ];
        Ok(Self {
            tabs,
            index,
            skimer,
            menu,
            display_settings,
            internal_settings,
        })
    }

    /// Returns a non mutable reference to the selected tab.
    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.index]
    }

    /// Returns a mutable reference to the selected tab.
    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.index]
    }

    /// Returns a string representing the current path in the selected tab.
    pub fn current_tab_path_str(&self) -> String {
        self.current_tab().directory_str()
    }

    /// True if a quit event was registered in the selected tab.
    pub fn must_quit(&self) -> bool {
        self.internal_settings.must_quit
    }

    /// Select the other tab if two are displayed. Does nother otherwise.
    pub fn next(&mut self) {
        if !self.display_settings.dual {
            return;
        }
        self.index = 1 - self.index
    }

    /// Select the other tab if two are displayed. Does nother otherwise.
    pub fn prev(&mut self) {
        self.next()
    }

    /// Select the left or right tab depending on where the user clicked.
    pub fn select_tab_from_col(&mut self, col: u16) -> Result<()> {
        let (width, _) = self.term_size()?;
        if self.display_settings.dual {
            if (col as usize) < width / 2 {
                self.select_left();
            } else {
                self.select_right();
            };
        } else {
            self.select_left();
        }
        Ok(())
    }

    pub fn window_from_row(&self, row: u16, height: usize) -> Window {
        let win_height = if matches!(self.current_tab().edit_mode, Edit::Nothing) {
            height
        } else {
            height / 2
        };
        log_info!("clicked row {row}, height {height}, win_height {win_height}");
        let w_index = row as usize / win_height;
        if w_index == 1 {
            Window::Menu
        } else if row == 1 {
            Window::Header
        } else if row as usize == win_height - 2 {
            Window::Footer
        } else {
            Window::Files
        }
    }

    pub fn click(&mut self, row: u16, col: u16, binds: &Bindings) -> Result<()> {
        let window = self.window_from_row(row, self.term_size()?.1);
        self.select_tab_from_col(col)?;
        let (_, current_height) = self.term_size()?;
        match window {
            Window::Menu => self.menu_action(row, current_height),
            Window::Header => self.header_action(col, binds)?,
            Window::Footer => self.footer_action(col, binds)?,
            Window::Files => {
                self.current_tab_mut().select_row(row, current_height)?;
                self.update_second_pane_for_preview()?;
            }
        };
        Ok(())
    }

    fn menu_action(&mut self, row: u16, height: usize) {
        let second_window_height = height / 2;
        let offset = row as usize - second_window_height;
        if offset >= 4 {
            let index = offset - 4;
            match self.current_tab().edit_mode {
                Edit::Navigate(navigate) => match navigate {
                    Navigate::Bulk => self.menu.bulk_set_index(index),
                    Navigate::CliApplication => self.menu.cli_applications.set_index(index),
                    Navigate::Compress => self.menu.compression.set_index(index),
                    Navigate::Context => self.menu.context.set_index(index),
                    Navigate::EncryptedDrive => self.menu.encrypted_devices.set_index(index),
                    Navigate::History => self.current_tab_mut().history.set_index(index),
                    Navigate::Jump => self.menu.flagged.set_index(index),
                    Navigate::Marks(_) => self.menu.marks.set_index(index),
                    Navigate::RemovableDevices => self.menu.removable_set_index(index),
                    Navigate::Shortcut => self.menu.shortcut.set_index(index),
                    Navigate::Trash => self.menu.trash.set_index(index),
                    Navigate::TuiApplication => self.menu.tui_applications.set_index(index),
                },
                Edit::InputCompleted(_) => self.menu.completion.set_index(index),
                _ => (),
            }
        }
    }

    pub fn select_left(&mut self) {
        self.index = 0;
    }

    pub fn select_right(&mut self) {
        self.index = 1;
    }

    /// Refresh every disk information.
    /// It also refreshes the disk list, which is usefull to detect removable medias.
    /// It may be very slow...
    /// There's surelly a better way, like doing it only once in a while or on
    /// demand.
    pub fn refresh_shortcuts(&mut self) {
        self.menu
            .refresh_shortcuts(&self.internal_settings.mount_points());
    }

    /// Returns an array of Disks
    pub fn disks(&self) -> &[Disk] {
        self.internal_settings.sys.disks()
    }

    /// Returns a the disk spaces for the selected tab..
    pub fn disk_spaces_of_selected(&self) -> String {
        disk_space(self.disks(), self.current_tab().current_path())
    }

    /// Returns the sice of the terminal (width, height)
    pub fn term_size(&self) -> Result<(usize, usize)> {
        self.internal_settings.term_size()
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn refresh_view(&mut self) -> Result<()> {
        self.menu.encrypted_devices.update()?;
        self.refresh_status()?;
        self.update_second_pane_for_preview()
    }

    /// Reset the view of every tab.
    pub fn reset_tabs_view(&mut self) -> Result<()> {
        for tab in self.tabs.iter_mut() {
            tab.refresh_and_reselect_file()?
        }
        Ok(())
    }

    fn reset_edit_mode(&mut self) {
        self.menu.completion.reset();
        self.current_tab_mut().reset_edit_mode();
    }
    /// Refresh the existing users.

    /// Reset the selected tab view to the default.
    pub fn refresh_status(&mut self) -> Result<()> {
        self.force_clear();
        self.refresh_users()?;
        self.refresh_tabs()?;
        Ok(())
    }

    /// Set a "force clear" flag to true, which will reset the display.
    /// It's used when some command or whatever may pollute the terminal.
    /// We ensure to clear it before displaying again.
    pub fn force_clear(&mut self) {
        self.internal_settings.force_clear();
    }

    pub fn refresh_users(&mut self) -> Result<()> {
        let users = Users::new();
        self.tabs[0].users = users.clone();
        self.tabs[1].users = users;
        Ok(())
    }

    pub fn refresh_tabs(&mut self) -> Result<()> {
        self.menu.input.reset();
        self.menu.completion.reset();
        self.tabs[0].refresh_and_reselect_file()?;
        self.tabs[1].refresh_and_reselect_file()
    }

    /// When a rezise event occurs, we may hide the second panel if the width
    /// isn't sufficiant to display enough information.
    /// We also need to know the new height of the terminal to start scrolling
    /// up or down.
    pub fn resize(&mut self, width: usize, height: usize) -> Result<()> {
        self.set_dual_pane_if_wide_enough(width)?;
        self.tabs[0].set_height(height);
        self.tabs[1].set_height(height);
        self.refresh_status()
    }

    /// Drop the current tree, replace it with an empty one.
    pub fn remove_tree(&mut self) -> Result<()> {
        self.current_tab_mut().tree = Tree::default();
        Ok(())
    }

    /// Check if the second pane should display a preview and force it.
    pub fn update_second_pane_for_preview(&mut self) -> Result<()> {
        if self.index == 0 && self.display_settings.preview {
            self.set_second_pane_for_preview()?;
        };
        Ok(())
    }

    /// Force preview the selected file of the first pane in the second pane.
    /// Doesn't check if it has do.
    pub fn set_second_pane_for_preview(&mut self) -> Result<()> {
        if !DisplaySettings::display_wide_enough(self.term_size()?.0) {
            self.tabs[1].preview = Preview::empty();
            return Ok(());
        }

        self.tabs[1].set_display_mode(Display::Preview);
        self.tabs[1].set_edit_mode(Edit::Nothing);
        let fileinfo = self.tabs[0]
            .current_file()
            .context("force preview: No file to select")?;
        let preview = match fileinfo.file_kind {
            FileKind::Directory => Preview::directory(&fileinfo, &self.tabs[0].users),
            _ => Preview::file(&fileinfo),
        };
        self.tabs[1].preview = preview.unwrap_or_default();

        self.tabs[1].window.reset(self.tabs[1].preview.len());
        Ok(())
    }

    /// Set dual pane if the term is big enough
    pub fn set_dual_pane_if_wide_enough(&mut self, width: usize) -> Result<()> {
        if width < MIN_WIDTH_FOR_DUAL_PANE {
            self.select_left();
            self.display_settings.dual = false;
        } else {
            self.display_settings.dual = true;
        }
        Ok(())
    }

    /// Empty the flagged files, reset the view of every tab.
    pub fn clear_flags_and_reset_view(&mut self) -> Result<()> {
        self.menu.flagged.clear();
        self.reset_tabs_view()
    }

    /// Returns a vector of path of files which are both flagged and in current
    /// directory.
    /// It's necessary since the user may have flagged files OUTSIDE of current
    /// directory before calling Bulkrename.
    /// It may be confusing since the same filename can be used in
    /// different places.
    pub fn flagged_in_current_dir(&self) -> Vec<&Path> {
        self.menu
            .flagged
            .in_current_dir(&self.current_tab().directory.path)
    }

    /// Flag all files in the current directory.
    pub fn flag_all(&mut self) {
        self.tabs[self.index]
            .directory
            .content
            .iter()
            .for_each(|file| {
                self.menu.flagged.push(file.path.to_path_buf());
            });
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn reverse_flags(&mut self) {
        self.tabs[self.index]
            .directory
            .content
            .iter()
            .for_each(|file| self.menu.flagged.toggle(&file.path));
    }

    /// Flag the selected file if any
    pub fn toggle_flag_for_selected(&mut self) {
        let tab = self.current_tab();

        if matches!(tab.edit_mode, Edit::Nothing) && !matches!(tab.display_mode, Display::Preview) {
            let Ok(file) = tab.current_file() else {
                return;
            };
            self.menu.flagged.toggle(&file.path);
            self.current_tab_mut().normal_down_one_row();
        };
    }

    /// Execute a move or a copy of the flagged files to current directory.
    /// A progress bar is displayed (invisible for small files) and a notification
    /// is sent every time, even for 0 bytes files...
    pub fn cut_or_copy_flagged_files(&mut self, cut_or_copy: CopyMove) -> Result<()> {
        let sources = self.menu.flagged.content.clone();

        let dest = &self.current_tab().directory_of_selected()?;

        copy_move(
            cut_or_copy,
            sources,
            dest,
            Arc::clone(&self.internal_settings.term),
        )?;
        self.clear_flags_and_reset_view()
    }

    fn skim_init(&mut self) {
        self.skimer = Some(Skimer::new(Arc::clone(&self.internal_settings.term)));
    }

    /// Replace the tab content with the first result of skim.
    /// It calls skim, reads its output, then update the tab content.
    pub fn skim_output_to_tab(&mut self) -> Result<()> {
        self.skim_init();
        let _ = self._skim_output_to_tab();
        self.drop_skim()
    }

    fn _skim_output_to_tab(&mut self) -> Result<()> {
        let Some(Ok(skimer)) = &self.skimer else {
            return Ok(());
        };
        let skim = skimer.search_filename(&self.current_tab().directory_str());
        let Some(output) = skim.first() else {
            return Ok(());
        };
        self._update_tab_from_skim_output(output)
    }

    /// Replace the tab content with the first result of skim.
    /// It calls skim, reads its output, then update the tab content.
    /// The output is splited at `:` since we only care about the path, not the line number.
    pub fn skim_line_output_to_tab(&mut self) -> Result<()> {
        self.skim_init();
        let _ = self._skim_line_output_to_tab();
        self.drop_skim()
    }

    fn _skim_line_output_to_tab(&mut self) -> Result<()> {
        let Some(Ok(skimer)) = &self.skimer else {
            return Ok(());
        };
        let skim = skimer.search_line_in_file(&self.current_tab().directory_str());
        let Some(output) = skim.first() else {
            return Ok(());
        };
        self._update_tab_from_skim_line_output(output)
    }

    /// Run a command directly from help.
    /// Search a command in skim, if it's a keybinding, run it directly.
    /// If the result can't be parsed, nothing is done.
    pub fn skim_find_keybinding_and_run(&mut self, help: String) -> Result<()> {
        self.skim_init();
        if let Ok(key) = self._skim_find_keybinding(help) {
            let _ = self.internal_settings.term.send_event(Event::Key(key));
        };
        self.drop_skim()
    }

    fn _skim_find_keybinding(&mut self, help: String) -> Result<tuikit::prelude::Key> {
        let Some(Ok(skimer)) = &mut self.skimer else {
            return Err(anyhow!("Skim isn't initialised"));
        };
        let skim = skimer.search_in_text(&help);
        let Some(output) = skim.first() else {
            return Err(anyhow!("Skim hasn't sent anything"));
        };
        let line = output.output().into_owned();
        let Some(keybind) = line.split(':').next() else {
            return Err(anyhow!("No keybind found"));
        };
        let Some(keyname) = parse_keyname(keybind) else {
            return Err(anyhow!("No keyname found for {keybind}"));
        };
        let Some(key) = from_keyname(&keyname) else {
            return Err(anyhow!("{keyname} isn't a valid Key name."));
        };
        Ok(key)
    }

    fn _update_tab_from_skim_line_output(&mut self, skim_output: &Arc<dyn SkimItem>) -> Result<()> {
        let output_str = skim_output.output().to_string();
        let Some(filename) = output_str.split(':').next() else {
            return Ok(());
        };
        let path = fs::canonicalize(filename)?;
        self._replace_path_by_skim_output(path)
    }

    fn _update_tab_from_skim_output(&mut self, skim_output: &Arc<dyn SkimItem>) -> Result<()> {
        let path = fs::canonicalize(skim_output.output().to_string())?;
        self._replace_path_by_skim_output(path)
    }

    fn _replace_path_by_skim_output(&mut self, path: std::path::PathBuf) -> Result<()> {
        let tab = self.current_tab_mut();
        if path.is_file() {
            let Some(parent) = path.parent() else {
                return Ok(());
            };
            tab.cd(parent)?;
            let filename = filename_from_path(&path)?;
            tab.search_from(filename, 0);
        } else if path.is_dir() {
            tab.cd(&path)?;
        }
        Ok(())
    }

    fn drop_skim(&mut self) -> Result<()> {
        self.skimer = None;
        Ok(())
    }

    pub fn execute_bulk(&self) -> Result<()> {
        if let Some(bulk) = &self.menu.bulk {
            bulk.execute_bulk(self)?;
        }
        Ok(())
    }

    pub fn input_regex(&mut self, char: char) -> Result<()> {
        self.menu.input.insert(char);
        self.select_from_regex()?;
        Ok(())
    }

    /// Flag every file matching a typed regex.
    pub fn select_from_regex(&mut self) -> Result<()> {
        let input = self.menu.input.string();
        if input.is_empty() {
            return Ok(());
        }
        let paths = match self.current_tab().display_mode {
            Display::Directory => self.tabs[self.index].directory.paths(),
            Display::Tree => self.tabs[self.index].tree.paths(),
            Display::Preview => return Ok(()),
        };
        regex_matcher(&input, &paths, &mut self.menu.flagged)?;
        Ok(())
    }

    /// Open a the selected file with its opener
    pub fn open_selected_file(&mut self) -> Result<()> {
        let path = self.current_tab().current_file()?.path;
        match self.internal_settings.opener.kind(&path) {
            Some(Kind::Internal(Internal::NotSupported)) => {
                let _ = self.mount_iso_drive();
            }
            Some(_) => {
                let _ = self.internal_settings.opener.open_single(&path);
            }
            None => (),
        }
        Ok(())
    }

    /// Open every flagged file with their respective opener.
    pub fn open_flagged_files(&mut self) -> Result<()> {
        self.internal_settings
            .opener
            .open_multiple(self.menu.flagged.content())
    }

    fn ensure_iso_device_is_some(&mut self) -> Result<()> {
        if self.menu.iso_device.is_none() {
            let path = path_to_string(&self.current_tab().current_file()?.path);
            self.menu.iso_device = Some(IsoDevice::from_path(path));
        }
        Ok(())
    }

    /// Mount the currently selected file (which should be an .iso file) to
    /// `/run/media/$CURRENT_USER/fm_iso`
    /// Ask a sudo password first if needed. It should always be the case.
    fn mount_iso_drive(&mut self) -> Result<()> {
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(Some(BlockDeviceAction::MOUNT), PasswordUsage::ISO)?;
        } else {
            self.ensure_iso_device_is_some()?;
            let Some(ref mut iso_device) = self.menu.iso_device else {
                return Ok(());
            };
            if iso_device.mount(&current_username()?, &mut self.menu.password_holder)? {
                log_info!("iso mounter mounted {iso_device:?}");
                log_line!("iso : {}", iso_device.as_string()?);
                let path = iso_device.mountpoints.clone().context("no mount point")?;
                self.current_tab_mut().cd(&path)?;
            };
            self.menu.iso_device = None;
        };

        Ok(())
    }

    /// Currently unused.
    /// Umount an iso device.
    pub fn umount_iso_drive(&mut self) -> Result<()> {
        if let Some(ref mut iso_device) = self.menu.iso_device {
            if !self.menu.password_holder.has_sudo() {
                self.ask_password(Some(BlockDeviceAction::UMOUNT), PasswordUsage::ISO)?;
            } else {
                iso_device.umount(&current_username()?, &mut self.menu.password_holder)?;
            };
        }
        Ok(())
    }

    /// Mount the selected encrypted device. Will ask first for sudo password and
    /// passphrase.
    /// Those passwords are always dropped immediatly after the commands are run.
    pub fn mount_encrypted_drive(&mut self) -> Result<()> {
        let Some(device) = self.menu.encrypted_devices.selected() else {
            return Ok(());
        };
        if device.is_mounted() {
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(
                Some(BlockDeviceAction::MOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::SUDO),
            )
        } else if !self.menu.password_holder.has_cryptsetup() {
            self.ask_password(
                Some(BlockDeviceAction::MOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP),
            )
        } else {
            self.menu
                .encrypted_devices
                .mount_selected(&mut self.menu.password_holder)
        }
    }

    /// Move to the selected crypted device mount point.
    pub fn go_to_encrypted_drive(&mut self) -> Result<()> {
        let Some(path) = self.menu.find_encrypted_drive_mount_point() else {
            return Ok(());
        };
        let tab = self.current_tab_mut();
        tab.cd(&path)?;
        tab.refresh_view()
    }

    /// Unmount the selected device.
    /// Will ask first for a sudo password which is immediatly forgotten.
    pub fn umount_encrypted_drive(&mut self) -> Result<()> {
        let Some(device) = self.menu.encrypted_devices.selected() else {
            return Ok(());
        };
        if !device.is_mounted() {
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(
                Some(BlockDeviceAction::UMOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::SUDO),
            )
        } else {
            self.menu
                .encrypted_devices
                .umount_selected(&mut self.menu.password_holder)
        }
    }

    pub fn go_to_removable(&mut self) -> Result<()> {
        let Some(path) = self.menu.find_removable_mount_point() else {
            return Ok(());
        };
        self.current_tab_mut().cd(&path)?;
        self.current_tab_mut().refresh_view()
    }

    pub fn parse_shell_command(&mut self) -> Result<bool> {
        let shell_command = self.menu.input.string();
        let mut args = ShellCommandParser::new(&shell_command).compute(self)?;
        log_info!("command {shell_command} args: {args:?}");
        if args_is_empty(&args) {
            self.current_tab_mut().set_edit_mode(Edit::Nothing);
            return Ok(true);
        }
        let executable = args.remove(0);
        if is_sudo_command(&executable) {
            self.menu.sudo_command = Some(shell_command);
            self.ask_password(None, PasswordUsage::SUDOCOMMAND)?;
            Ok(false)
        } else {
            if !is_program_in_path(&executable) {
                return Ok(true);
            }
            let current_directory = self.current_tab().directory_of_selected()?.to_owned();
            let params: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            execute_without_output_with_path(executable, current_directory, Some(&params))?;
            self.current_tab_mut().set_edit_mode(Edit::Nothing);
            Ok(true)
        }
    }

    /// Ask for a password of some kind (sudo or device passphrase).
    fn ask_password(
        &mut self,
        encrypted_action: Option<BlockDeviceAction>,
        password_dest: PasswordUsage,
    ) -> Result<()> {
        log_info!("event ask password");
        self.current_tab_mut()
            .set_edit_mode(Edit::InputSimple(InputSimple::Password(
                encrypted_action,
                password_dest,
            )));
        Ok(())
    }

    pub fn execute_password_command(&mut self) -> Result<()> {
        match self.current_tab().edit_mode {
            Edit::InputSimple(InputSimple::Password(action, dest)) => {
                self._execute_password_command(action, dest)?;
            }
            _ => {
                return Err(anyhow!(
                    "execute_password_command: edit_mode should be `InputSimple::Password`"
                ))
            }
        }
        Ok(())
    }

    fn _execute_password_command(
        &mut self,
        action: Option<BlockDeviceAction>,
        dest: PasswordUsage,
    ) -> Result<()> {
        let password = self.menu.input.string();
        self.menu.input.reset();
        match dest {
            PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP) => {
                self.menu.password_holder.set_cryptsetup(password)
            }
            _ => self.menu.password_holder.set_sudo(password),
        };
        self.reset_edit_mode();
        self.dispatch_password(dest, action)
    }

    /// Execute a new mark, saving it to a config file for futher use.
    pub fn marks_new(&mut self, c: char) -> Result<()> {
        let path = self.current_tab_mut().directory.path.clone();
        self.menu.marks.new_mark(c, &path)?;
        {
            let tab: &mut Tab = self.current_tab_mut();
            tab.refresh_view()
        }?;
        self.reset_edit_mode();
        self.refresh_status()
    }

    /// Execute a jump to a mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    pub fn marks_jump_char(&mut self, c: char) -> Result<()> {
        if let Some(path) = self.menu.marks.get(c) {
            self.current_tab_mut().cd(&path)?;
        }
        self.current_tab_mut().refresh_view()?;
        self.reset_edit_mode();
        self.refresh_status()
    }

    /// Recursively delete all flagged files.
    pub fn confirm_delete_files(&mut self) -> Result<()> {
        self.menu.delete_flagged_files()?;
        self.reset_edit_mode();
        self.clear_flags_and_reset_view()?;
        self.refresh_status()
    }

    /// Empty the trash folder permanently.
    pub fn confirm_trash_empty(&mut self) -> Result<()> {
        self.menu.trash.empty_trash()?;
        self.reset_edit_mode();
        self.clear_flags_and_reset_view()?;
        Ok(())
    }

    fn run_sudo_command(&mut self) -> Result<()> {
        self.current_tab_mut().set_edit_mode(Edit::Nothing);
        reset_sudo_faillock()?;
        let Some(sudo_command) = &self.menu.sudo_command else {
            return self.menu.clear_sudo_attributes();
        };
        let args = ShellCommandParser::new(sudo_command).compute(self)?;
        if args.is_empty() {
            return self.menu.clear_sudo_attributes();
        }
        execute_sudo_command_with_password(
            &args[1..],
            self.menu
                .password_holder
                .sudo()
                .as_ref()
                .context("sudo password isn't set")?,
            self.current_tab().directory_of_selected()?,
        )?;
        self.menu.clear_sudo_attributes()?;
        self.refresh_status()
    }

    pub fn dispatch_password(
        &mut self,
        dest: PasswordUsage,
        action: Option<BlockDeviceAction>,
    ) -> Result<()> {
        match dest {
            PasswordUsage::ISO => match action {
                Some(BlockDeviceAction::MOUNT) => self.mount_iso_drive(),
                Some(BlockDeviceAction::UMOUNT) => self.umount_iso_drive(),
                None => Ok(()),
            },
            PasswordUsage::CRYPTSETUP(_) => match action {
                Some(BlockDeviceAction::MOUNT) => self.mount_encrypted_drive(),
                Some(BlockDeviceAction::UMOUNT) => self.umount_encrypted_drive(),
                None => Ok(()),
            },
            PasswordUsage::SUDOCOMMAND => self.run_sudo_command(),
        }
    }

    pub fn update_nvim_listen_address(&mut self) {
        self.internal_settings.update_nvim_listen_address()
    }

    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    /// The action is only executed if the user typed the char `y`
    pub fn confirm(&mut self, c: char, confirmed_action: NeedConfirmation) -> Result<()> {
        if c == 'y' {
            let _ = self.match_confirmed_mode(confirmed_action);
        }
        self.reset_edit_mode();
        self.current_tab_mut().refresh_view()?;

        Ok(())
    }

    fn match_confirmed_mode(&mut self, confirmed_action: NeedConfirmation) -> Result<()> {
        match confirmed_action {
            NeedConfirmation::Delete => self.confirm_delete_files(),
            NeedConfirmation::Move => self.cut_or_copy_flagged_files(CopyMove::Move),
            NeedConfirmation::Copy => self.cut_or_copy_flagged_files(CopyMove::Copy),
            NeedConfirmation::EmptyTrash => self.confirm_trash_empty(),
        }
    }

    pub fn header_action(&mut self, col: u16, binds: &Bindings) -> Result<()> {
        if matches!(self.current_tab().display_mode, Display::Preview) {
            return Ok(());
        }
        let is_right = self.index == 1;
        let header = Header::new(self, self.current_tab())?;
        let action = header.action(col as usize, is_right);
        action.matcher(self, binds)
    }

    pub fn footer_action(&mut self, col: u16, binds: &Bindings) -> Result<()> {
        log_info!("footer clicked col {col}");
        if matches!(self.current_tab().display_mode, Display::Preview) {
            return Ok(());
        }
        let is_right = self.index == 1;
        let footer = Footer::new(self, self.current_tab())?;
        let action = footer.action(col as usize, is_right);
        action.matcher(self, binds)
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    pub fn chmod(&mut self) -> Result<()> {
        if self.menu.input.is_empty() || self.menu.flagged.is_empty() {
            return Ok(());
        }
        let input_permission = &self.menu.input.string();
        Permissions::set_permissions_of_flagged(input_permission, &mut self.menu.flagged)?;
        self.reset_tabs_view()
    }

    pub fn set_mode_chmod(&mut self) -> Result<()> {
        if self.current_tab_mut().directory.is_empty() {
            return Ok(());
        }
        self.current_tab_mut()
            .set_edit_mode(Edit::InputSimple(InputSimple::Chmod));
        if self.menu.flagged.is_empty() {
            self.toggle_flag_for_selected();
        };
        Ok(())
    }

    /// Add a char to input string, look for a possible completion.
    pub fn input_complete(&mut self, c: char) -> Result<()> {
        self.menu.input_complete(c, &self.tabs[self.index])
    }

    /// Execute a custom event on the selected file
    pub fn run_custom_command(&mut self, string: &str) -> Result<()> {
        log_info!("custom {string}");
        let parser = ShellCommandParser::new(string);
        let mut args = parser.compute(self)?;
        let command = args.remove(0);
        let args: Vec<&str> = args.iter().map(|s| &**s).collect();
        let output = execute_and_capture_output_without_check(command, &args)?;
        log_info!("output {output}");
        Ok(())
    }
}

fn parse_keyname(keyname: &str) -> Option<String> {
    let mut split = keyname.split('(');
    let Some(mutator) = split.next() else {
        return None;
    };
    let mut mutator = mutator.to_lowercase();
    let Some(param) = split.next() else {
        return Some(mutator);
    };
    let mut param = param.trim().to_owned();
    mutator = mutator.replace("char", "");
    param = param.replace([')', '\''], "");
    if param.chars().all(char::is_uppercase) {
        if mutator.is_empty() {
            mutator = "shift".to_owned();
        } else {
            mutator = format!("{mutator}-shift");
        }
    }

    if mutator.is_empty() {
        Some(param)
    } else {
        Some(format!("{mutator}-{param}"))
    }
}
