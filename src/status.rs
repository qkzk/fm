use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use log::info;
use regex::Regex;
use skim::SkimItem;
use sysinfo::{Disk, DiskExt, RefreshKind, System, SystemExt};
use tuikit::prelude::{from_keyname, Event};
use tuikit::term::Term;

use crate::args::Args;
use crate::bulkrename::Bulk;
use crate::cli_info::CliInfo;
use crate::compress::Compresser;
use crate::config::Settings;
use crate::constant_strings_paths::{NVIM, SS, TUIS_PATH};
use crate::copy_move::{copy_move, CopyMove};
use crate::cryptsetup::{BlockDeviceAction, CryptoDeviceOpener};
use crate::fileinfo::FileKind;
use crate::flagged::Flagged;
use crate::iso::IsoDevice;
use crate::log_line;
use crate::marks::Marks;
use crate::mode::{DisplayMode, EditMode, InputSimple, NeedConfirmation};
use crate::mount_help::MountHelper;
use crate::opener::{InternalVariant, Opener};
use crate::password::{
    drop_sudo_privileges, execute_sudo_command_with_password, reset_sudo_faillock, PasswordHolder,
    PasswordKind, PasswordUsage,
};
use crate::preview::Preview;
use crate::removable_devices::RemovableDevices;
use crate::selectable_content::SelectableContent;
use crate::shell_menu::ShellMenu;
use crate::shell_parser::ShellCommandParser;
use crate::skim::Skimer;
use crate::tab::Tab;
use crate::term_manager::MIN_WIDTH_FOR_DUAL_PANE;
use crate::trash::Trash;
use crate::tree::Tree;
use crate::users::Users;
use crate::utils::{current_username, disk_space, filename_from_path, is_program_in_path};

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
    /// The flagged files
    pub flagged: Flagged,
    /// Marks allows you to jump to a save mark
    pub marks: Marks,
    /// terminal
    pub term: Arc<Term>,
    skimer: Option<Result<Skimer>>,
    /// do we display one or two tabs ?
    pub dual_pane: bool,
    pub system_info: System,
    /// do we display all info or only the filenames ?
    pub display_full: bool,
    /// use the second pane to preview auto
    pub preview_second: bool,
    /// The opener used by the application.
    pub opener: Opener,
    /// The help string.
    pub help: String,
    /// The trash
    pub trash: Trash,
    /// Encrypted devices opener
    pub encrypted_devices: CryptoDeviceOpener,
    /// Iso mounter. Set to None by default, dropped ASAP
    pub iso_device: Option<IsoDevice>,
    /// Compression methods
    pub compression: Compresser,
    /// NVIM RPC server address
    pub nvim_server: String,
    pub force_clear: bool,
    pub bulk: Option<Bulk>,
    pub shell_menu: ShellMenu,
    pub cli_info: CliInfo,
    pub start_folder: std::path::PathBuf,
    pub password_holder: PasswordHolder,
    pub sudo_command: Option<String>,
    pub removable_devices: Option<RemovableDevices>,
}

impl Status {
    /// Max valid permission number, ie `0o777`.
    pub const MAX_PERMISSIONS: u32 = 0o777;

    /// Creates a new status for the application.
    /// It requires most of the information (arguments, configuration, height
    /// of the terminal, the formated help string).
    pub fn new(
        height: usize,
        term: Arc<Term>,
        help: String,
        opener: Opener,
        settings: &Settings,
    ) -> Result<Self> {
        let args = Args::parse();
        let preview_second = args.preview;
        let start_folder = std::fs::canonicalize(std::path::PathBuf::from(&args.path))?;
        let nvim_server = args.server.clone();
        let display_full = Self::parse_display_full(args.simple, settings.full);
        let dual_pane = Self::parse_dual_pane(args.dual, settings.dual, &term)?;

        let Ok(shell_menu) = ShellMenu::new(TUIS_PATH) else {
            Self::quit()
        };
        let cli_info = CliInfo::default();
        let sys = System::new_with_specifics(RefreshKind::new().with_disks());
        let encrypted_devices = CryptoDeviceOpener::default();
        let trash = Trash::new()?;
        let compression = Compresser::default();
        let force_clear = false;
        let bulk = None;
        let iso_device = None;
        let password_holder = PasswordHolder::default();
        let sudo_command = None;
        let flagged = Flagged::default();
        let marks = Marks::read_from_config_file();
        let skimer = None;
        let index = 0;

        // unsafe because of UsersCache::with_all_users
        let users = Users::new();
        // unsafe because of UsersCache::with_all_users
        let users2 = users.clone();

        let mount_points = Self::disks_mounts(sys.disks());

        let tabs = [
            Tab::new(&args, height, users, settings, &mount_points)?,
            Tab::new(&args, height, users2, settings, &mount_points)?,
        ];
        let removable_devices = None;
        Ok(Self {
            tabs,
            index,
            flagged,
            marks,
            skimer,
            term,
            dual_pane,
            preview_second,
            system_info: sys,
            display_full,
            opener,
            help,
            trash,
            encrypted_devices,
            compression,
            nvim_server,
            force_clear,
            bulk,
            shell_menu,
            iso_device,
            cli_info,
            start_folder,
            password_holder,
            sudo_command,
            removable_devices,
        })
    }

    /// Creats a new bulk instance if needed
    pub fn init_bulk(&mut self) {
        if self.bulk.is_none() {
            self.bulk = Some(Bulk::default());
        }
    }

    pub fn bulk_prev(&mut self) {
        self.init_bulk();
        if let Some(bulk) = &mut self.bulk {
            bulk.prev();
        }
    }

    pub fn bulk_next(&mut self) {
        self.init_bulk();
        if let Some(bulk) = &mut self.bulk {
            bulk.next();
        }
    }

    pub fn execute_bulk(&self) -> Result<()> {
        if let Some(bulk) = &self.bulk {
            bulk.execute_bulk(self)?;
        }
        Ok(())
    }

    fn display_wide_enough(term: &Arc<Term>) -> Result<bool> {
        Ok(term.term_size()?.0 >= MIN_WIDTH_FOR_DUAL_PANE)
    }

    fn quit() -> ! {
        eprintln!("Couldn't load the TUIs config file at {TUIS_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/tuis.yaml for an example");
        info!("Couldn't read tuis file at {TUIS_PATH}. Exiting");
        std::process::exit(1);
    }

    fn parse_display_full(simple_args: Option<bool>, full_config: bool) -> bool {
        if let Some(simple_args) = simple_args {
            return !simple_args;
        }
        full_config
    }

    fn parse_dual_pane(
        args_dual: Option<bool>,
        dual_config: bool,
        term: &Arc<Term>,
    ) -> Result<bool> {
        if !Self::display_wide_enough(term)? {
            return Ok(false);
        }
        if let Some(args_dual) = args_dual {
            return Ok(args_dual);
        }
        Ok(dual_config)
    }
    /// Select the other tab if two are displayed. Does nother otherwise.
    pub fn next(&mut self) {
        if !self.dual_pane {
            return;
        }
        self.index = 1 - self.index
    }

    /// Select the other tab if two are displayed. Does nother otherwise.
    pub fn prev(&mut self) {
        self.next()
    }

    /// Returns a mutable reference to the selected tab.
    pub fn selected(&mut self) -> &mut Tab {
        &mut self.tabs[self.index]
    }

    /// Returns a non mutable reference to the selected tab.
    pub fn selected_non_mut(&self) -> &Tab {
        &self.tabs[self.index]
    }

    /// Reset the view of every tab.
    pub fn reset_tabs_view(&mut self) -> Result<()> {
        for tab in self.tabs.iter_mut() {
            tab.refresh_view()?
        }
        Ok(())
    }

    /// Toggle the flagged attribute of a path.
    pub fn toggle_flag_on_path(&mut self, path: &Path) {
        self.flagged.toggle(path)
    }

    fn skim_init(&mut self) {
        self.skimer = Some(Skimer::new(Arc::clone(&self.term)));
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
        let skim = skimer.search_filename(
            self.selected_non_mut()
                .path_content_str()
                .context("Couldn't parse current directory")?,
        );
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
        let skim = skimer.search_line_in_file(
            self.selected_non_mut()
                .path_content_str()
                .context("Couldn't parse current directory")?,
        );
        let Some(output) = skim.first() else {
            return Ok(());
        };
        self._update_tab_from_skim_line_output(output)
    }

    /// Run a command directly from help.
    /// Search a command in skim, if it's a keybinding, run it directly.
    /// If the result can't be parsed, nothing is done.
    pub fn skim_find_keybinding(&mut self) -> Result<()> {
        self.skim_init();
        self._skim_find_keybinding();
        self.drop_skim()
    }

    fn _skim_find_keybinding(&mut self) {
        let Some(Ok(skimer)) = &mut self.skimer else {
            return;
        };
        let skim = skimer.search_in_text(&self.help);
        let Some(output) = skim.first() else {
            return;
        };
        let line = output.output().into_owned();
        let Some(keybind) = line.split(':').next() else {
            return;
        };
        let Some(keyname) = parse_keyname(keybind) else {
            return;
        };
        let Some(key) = from_keyname(&keyname) else {
            return;
        };
        let event = Event::Key(key);
        let _ = self.term.send_event(event);
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
        let tab = self.selected();
        if path.is_file() {
            let Some(parent) = path.parent() else {
                return Ok(());
            };
            tab.set_pathcontent(parent)?;
            let filename = filename_from_path(&path)?;
            tab.search_from(filename, 0);
        } else if path.is_dir() {
            tab.set_pathcontent(&path)?;
        }
        Ok(())
    }

    fn drop_skim(&mut self) -> Result<()> {
        self.skimer = None;
        Ok(())
    }

    /// Returns a vector of path of files which are both flagged and in current
    /// directory.
    /// It's necessary since the user may have flagged files OUTSIDE of current
    /// directory before calling Bulkrename.
    /// It may be confusing since the same filename can be used in
    /// different places.
    pub fn filtered_flagged_files(&self) -> Vec<&Path> {
        self.flagged
            .filtered(&self.selected_non_mut().path_content.path)
    }

    /// Execute a move or a copy of the flagged files to current directory.
    /// A progress bar is displayed (invisible for small files) and a notification
    /// is sent every time, even for 0 bytes files...
    pub fn cut_or_copy_flagged_files(&mut self, cut_or_copy: CopyMove) -> Result<()> {
        let sources = self.flagged.content.clone();

        let dest = &self.selected_non_mut().directory_of_selected()?;

        copy_move(cut_or_copy, sources, dest, Arc::clone(&self.term))?;
        self.clear_flags_and_reset_view()
    }

    /// Empty the flagged files, reset the view of every tab.
    pub fn clear_flags_and_reset_view(&mut self) -> Result<()> {
        self.flagged.clear();
        self.reset_tabs_view()
    }

    /// Remove a flag file from Jump mode
    pub fn jump_remove_selected_flagged(&mut self) -> Result<()> {
        self.flagged.remove_selected();
        Ok(())
    }

    pub fn trash_delete_permanently(&mut self) -> Result<()> {
        self.trash.delete_permanently()
    }
    /// Move the selected flagged file to the trash.
    pub fn trash_single_flagged(&mut self) -> Result<()> {
        let filepath = self
            .flagged
            .selected()
            .context("no flagged file")?
            .to_owned();
        self.flagged.remove_selected();
        self.trash.trash(&filepath)?;
        Ok(())
    }

    /// Delete the selected flagged file.
    pub fn delete_single_flagged(&mut self) -> Result<()> {
        let filepath = self
            .flagged
            .selected()
            .context("no flagged file")?
            .to_owned();
        self.flagged.remove_selected();
        if filepath.is_dir() {
            std::fs::remove_dir_all(filepath)?;
        } else {
            std::fs::remove_file(filepath)?;
        }
        Ok(())
    }

    pub fn click(&mut self, row: u16, col: u16, current_height: usize) -> Result<()> {
        self.select_pane(col)?;
        self.selected().select_row(row, current_height)
    }

    /// Set the permissions of the flagged files according to a given permission.
    /// If the permission are invalid or if the user can't edit them, it may fail.
    pub fn set_permissions<P>(path: P, permissions: u32) -> Result<()>
    where
        P: AsRef<Path>,
    {
        Ok(std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(permissions),
        )?)
    }

    pub fn input_regex(&mut self, char: char) -> Result<()> {
        self.selected().input.insert(char);
        self.select_from_regex()?;
        Ok(())
    }

    /// Flag every file matching a typed regex.
    pub fn select_from_regex(&mut self) -> Result<(), regex::Error> {
        if self.selected_non_mut().input.string().is_empty() {
            return Ok(());
        }
        self.flagged.clear();
        let re = Regex::new(&self.selected_non_mut().input.string())?;
        for file in self.tabs[self.index].path_content.content.iter() {
            if re.is_match(&file.path.to_string_lossy()) {
                self.flagged.push(file.path.clone());
            }
        }
        Ok(())
    }

    /// Select a tab according to its index.
    /// It's deprecated and is left mostly because I'm not sure I want
    /// tabs & panes... and I haven't fully decided yet.
    /// Since I'm lazy and don't want to write it twice, it's left here.
    pub fn select_tab(&mut self, index: usize) -> Result<()> {
        if index >= self.tabs.len() {
            Err(anyhow!(
                "Only {} tabs. Can't select tab {}",
                self.tabs.len(),
                index
            ))
        } else {
            self.index = index;
            Ok(())
        }
    }

    /// Refresh every disk information.
    /// It also refreshes the disk list, which is usefull to detect removable medias.
    /// It may be very slow...
    /// There's surelly a better way, like doing it only once in a while or on
    /// demand.
    pub fn refresh_disks(&mut self) {
        // the fast variant, which doesn't check if the disks have changed.
        // self.system_info.refresh_disks();

        // the slow variant, which check if the disks have changed.
        self.system_info.refresh_disks_list();
        let disks = self.system_info.disks();
        let mounts = Self::disks_mounts(disks);
        self.tabs[0].refresh_shortcuts(&mounts);
        self.tabs[1].refresh_shortcuts(&mounts);
    }

    /// Returns an array of Disks
    pub fn disks(&self) -> &[Disk] {
        self.system_info.disks()
    }

    /// Returns a pair of disk spaces for both tab.
    pub fn disk_spaces_per_tab(&self) -> (String, String) {
        let disks = self.disks();
        (
            disk_space(disks, &self.tabs[0].path_content.path),
            disk_space(disks, &self.tabs[1].path_content.path),
        )
    }

    /// Returns the mount points of every disk.
    pub fn disks_mounts(disks: &[Disk]) -> Vec<&Path> {
        disks.iter().map(|d| d.mount_point()).collect()
    }

    /// Returns the sice of the terminal (width, height)
    pub fn term_size(&self) -> Result<(usize, usize)> {
        Ok(self.term.term_size()?)
    }

    /// Returns a string representing the current path in the selected tab.
    pub fn selected_path_str(&self) -> &str {
        self.selected_non_mut()
            .path_content_str()
            .unwrap_or_default()
    }

    /// Refresh the existing users.
    pub fn refresh_users(&mut self) -> Result<()> {
        let users = Users::new();
        self.tabs[0].users = users.clone();
        self.tabs[1].users = users;
        self.tabs[0].refresh_view()?;
        self.tabs[1].refresh_view()?;
        Ok(())
    }

    /// Drop the current tree, replace it with an empty one.
    pub fn remove_tree(&mut self) -> Result<()> {
        self.selected().tree = Tree::default();
        Ok(())
    }

    /// Updates the encrypted devices
    pub fn read_encrypted_devices(&mut self) -> Result<()> {
        self.encrypted_devices.update()?;
        Ok(())
    }

    pub fn make_preview(&mut self) -> Result<()> {
        if self.selected_non_mut().path_content.is_empty() {
            return Ok(());
        }
        let Ok(file_info) = self.selected_non_mut().selected() else {
            return Ok(());
        };
        match file_info.file_kind {
            FileKind::NormalFile => {
                let preview = Preview::file(&file_info).unwrap_or_default();
                self.selected().set_display_mode(DisplayMode::Preview);
                self.selected().window.reset(preview.len());
                self.selected().preview = preview;
            }
            FileKind::Directory => self.tree()?,
            _ => (),
        }

        Ok(())
    }

    pub fn tree(&mut self) -> Result<()> {
        if let DisplayMode::Tree = self.selected_non_mut().display_mode {
            {
                let tab = self.selected();
                tab.tree = Tree::default();
                tab.refresh_view()
            }?;
            self.selected().set_display_mode(DisplayMode::Normal)
        } else {
            self.display_full = true;
            self.selected().make_tree(None)?;
            self.selected().set_display_mode(DisplayMode::Tree);
        }
        Ok(())
    }

    /// Check if the second pane should display a preview and force it.
    pub fn update_second_pane_for_preview(&mut self) -> Result<()> {
        if self.index == 0 && self.preview_second {
            self.set_second_pane_for_preview()?;
        };
        Ok(())
    }

    /// Force preview the selected file of the first pane in the second pane.
    /// Doesn't check if it has do.
    pub fn set_second_pane_for_preview(&mut self) -> Result<()> {
        if !Self::display_wide_enough(&self.term)? {
            self.tabs[1].preview = Preview::empty();
            return Ok(());
        }

        self.tabs[1].set_display_mode(DisplayMode::Preview);
        self.tabs[1].set_edit_mode(EditMode::Nothing);
        let fileinfo = self.tabs[0]
            .selected()
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
            self.select_tab(0)?;
            self.dual_pane = false;
        } else {
            self.dual_pane = true;
        }
        Ok(())
    }

    /// True if a quit event was registered in the selected tab.
    pub fn must_quit(&self) -> bool {
        self.selected_non_mut().must_quit()
    }

    /// Set a "force clear" flag to true, which will reset the display.
    /// It's used when some command or whatever may pollute the terminal.
    /// We ensure to clear it before displaying again.
    pub fn force_clear(&mut self) {
        self.force_clear = true;
    }

    /// Open a the selected file with its opener
    pub fn open_selected_file(&mut self) -> Result<()> {
        let filepath = if matches!(self.selected_non_mut().display_mode, DisplayMode::Tree) {
            self.selected_non_mut().tree.selected_path().to_owned()
        } else {
            self.selected_non_mut().selected()?.path.to_owned()
        };
        let opener = self.opener.open_info(&filepath);
        if let Some(InternalVariant::NotSupported) = opener.internal_variant.as_ref() {
            self.mount_iso_drive()?;
        } else {
            match self.opener.open(&filepath) {
                Ok(_) => (),
                Err(e) => info!(
                    "Error opening {:?}: {:?}",
                    self.selected_non_mut().path_content.selected(),
                    e
                ),
            }
        }
        Ok(())
    }

    /// Open every flagged file with their respective opener.
    pub fn open_flagged_files(&mut self) -> Result<()> {
        self.opener.open_multiple(self.flagged.content())
    }

    /// Mount the currently selected file (which should be an .iso file) to
    /// `/run/media/$CURRENT_USER/fm_iso`
    /// Ask a sudo password first if needed. It should always be the case.
    pub fn mount_iso_drive(&mut self) -> Result<()> {
        let path = self
            .selected_non_mut()
            .path_content
            .selected_path_string()
            .context("Couldn't parse the path")?;
        if self.iso_device.is_none() {
            self.iso_device = Some(IsoDevice::from_path(path));
        }
        if let Some(ref mut iso_device) = self.iso_device {
            if !self.password_holder.has_sudo() {
                Self::ask_password(
                    self,
                    PasswordKind::SUDO,
                    Some(BlockDeviceAction::MOUNT),
                    PasswordUsage::ISO,
                )?;
            } else {
                if iso_device.mount(&current_username()?, &mut self.password_holder)? {
                    info!("iso mounter mounted {iso_device:?}");

                    log_line!("iso : {}", iso_device.as_string()?);
                    let path = iso_device.mountpoints.clone().context("no mount point")?;
                    self.selected().set_pathcontent(&path)?;
                };
                self.iso_device = None;
            };
        }
        Ok(())
    }

    /// Currently unused.
    /// Umount an iso device.
    pub fn umount_iso_drive(&mut self) -> Result<()> {
        if let Some(ref mut iso_device) = self.iso_device {
            if !self.password_holder.has_sudo() {
                Self::ask_password(
                    self,
                    PasswordKind::SUDO,
                    Some(BlockDeviceAction::UMOUNT),
                    PasswordUsage::ISO,
                )?;
            } else {
                iso_device.umount(&current_username()?, &mut self.password_holder)?;
            };
        }
        Ok(())
    }

    /// Mount the selected encrypted device. Will ask first for sudo password and
    /// passphrase.
    /// Those passwords are always dropped immediatly after the commands are run.
    pub fn mount_encrypted_drive(&mut self) -> Result<()> {
        let Some(device) = self.encrypted_devices.selected() else {
            return Ok(());
        };
        if device.is_mounted() {
            return Ok(());
        }
        if !self.password_holder.has_sudo() {
            Self::ask_password(
                self,
                PasswordKind::SUDO,
                Some(BlockDeviceAction::MOUNT),
                PasswordUsage::CRYPTSETUP,
            )
        } else if !self.password_holder.has_cryptsetup() {
            Self::ask_password(
                self,
                PasswordKind::CRYPTSETUP,
                Some(BlockDeviceAction::MOUNT),
                PasswordUsage::CRYPTSETUP,
            )
        } else {
            self.encrypted_devices
                .mount_selected(&mut self.password_holder)
        }
    }

    /// Move to the selected crypted device mount point.
    pub fn go_to_encrypted_drive(&mut self) -> Result<()> {
        let Some(device) = self.encrypted_devices.selected() else {
            return Ok(());
        };
        if !device.is_mounted() {
            return Ok(());
        }
        let Some(mount_point) = device.mount_point() else {
            return Ok(());
        };
        let tab = self.selected();
        let path = std::path::PathBuf::from(mount_point);
        tab.set_pathcontent(&path)?;
        tab.refresh_view()
    }

    /// Unmount the selected device.
    /// Will ask first for a sudo password which is immediatly forgotten.
    pub fn umount_encrypted_drive(&mut self) -> Result<()> {
        let Some(device) = self.encrypted_devices.selected() else {
            return Ok(());
        };
        if !device.is_mounted() {
            return Ok(());
        }
        if !self.password_holder.has_sudo() {
            Self::ask_password(
                self,
                PasswordKind::SUDO,
                Some(BlockDeviceAction::UMOUNT),
                PasswordUsage::CRYPTSETUP,
            )
        } else {
            self.encrypted_devices
                .umount_selected(&mut self.password_holder)
        }
    }

    pub fn mount_removable(&mut self) -> Result<()> {
        let Some(devices) = &mut self.removable_devices else {
            return Ok(());
        };
        let Some(device) = devices.selected_mut() else {
            return Ok(());
        };
        if device.is_mounted() {
            return Ok(());
        }
        device.mount_simple()?;
        Ok(())
    }

    pub fn umount_removable(&mut self) -> Result<()> {
        let Some(devices) = &mut self.removable_devices else {
            return Ok(());
        };
        let Some(device) = devices.selected_mut() else {
            return Ok(());
        };
        if !device.is_mounted() {
            return Ok(());
        }
        device.umount_simple()?;
        Ok(())
    }

    pub fn go_to_removable(&mut self) -> Result<()> {
        let Some(devices) = &self.removable_devices else {
            return Ok(());
        };
        let Some(device) = devices.selected() else {
            return Ok(());
        };
        if !device.is_mounted() {
            return Ok(());
        }
        let path = std::path::PathBuf::from(&device.path);
        self.selected().set_pathcontent(&path)?;
        self.selected().refresh_view()
    }

    /// Ask for a password of some kind (sudo or device passphrase).
    pub fn ask_password(
        &mut self,
        password_kind: PasswordKind,
        encrypted_action: Option<BlockDeviceAction>,
        password_dest: PasswordUsage,
    ) -> Result<()> {
        info!("event ask password");
        self.selected()
            .set_edit_mode(EditMode::InputSimple(InputSimple::Password(
                password_kind,
                encrypted_action,
                password_dest,
            )));
        Ok(())
    }

    /// Execute a new mark, saving it to a config file for futher use.
    pub fn marks_new(&mut self, c: char) -> Result<()> {
        let path = self.selected().path_content.path.clone();
        self.marks.new_mark(c, &path)?;
        {
            let tab: &mut Tab = self.selected();
            tab.refresh_view()
        }?;
        self.selected().reset_edit_mode();
        self.refresh_status()
    }

    /// Execute a jump to a mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    pub fn marks_jump_char(&mut self, c: char) -> Result<()> {
        if let Some(path) = self.marks.get(c) {
            self.selected().set_pathcontent(&path)?;
        }
        self.selected().refresh_view()?;
        self.selected().reset_edit_mode();
        self.refresh_status()
    }

    /// Reset the selected tab view to the default.
    pub fn refresh_status(&mut self) -> Result<()> {
        self.force_clear();
        self.refresh_users()?;
        self.selected().refresh_view()?;
        if let DisplayMode::Tree = self.selected_non_mut().display_mode {
            self.selected().make_tree(None)?
        }
        Ok(())
    }

    /// When a rezise event occurs, we may hide the second panel if the width
    /// isn't sufficiant to display enough information.
    /// We also need to know the new height of the terminal to start scrolling
    /// up or down.
    pub fn resize(&mut self, width: usize, height: usize) -> Result<()> {
        self.set_dual_pane_if_wide_enough(width)?;
        self.selected().set_height(height);
        self.force_clear();
        self.refresh_users()?;
        Ok(())
    }

    /// Recursively delete all flagged files.
    pub fn confirm_delete_files(&mut self) -> Result<()> {
        let nb = self.flagged.len();
        for pathbuf in self.flagged.content.iter() {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(pathbuf)?;
            } else {
                std::fs::remove_file(pathbuf)?;
            }
        }
        log_line!("Deleted {nb} flagged files");
        self.selected().reset_edit_mode();
        self.clear_flags_and_reset_view()?;
        self.refresh_status()
    }

    /// Empty the trash folder permanently.
    pub fn confirm_trash_empty(&mut self) -> Result<()> {
        self.trash.empty_trash()?;
        self.selected().reset_edit_mode();
        self.clear_flags_and_reset_view()?;
        Ok(())
    }

    fn run_sudo_command(&mut self) -> Result<()> {
        self.selected().set_edit_mode(EditMode::Nothing);
        reset_sudo_faillock()?;
        let Some(sudo_command) = &self.sudo_command else {
            return Ok(());
        };
        let args = ShellCommandParser::new(sudo_command).compute(self)?;
        if args.is_empty() {
            return Ok(());
        }
        execute_sudo_command_with_password(
            &args[1..],
            &self.password_holder.sudo()?,
            self.selected_non_mut().directory_of_selected()?,
        )?;
        self.password_holder.reset();
        drop_sudo_privileges()?;
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
            PasswordUsage::CRYPTSETUP => match action {
                Some(BlockDeviceAction::MOUNT) => self.mount_encrypted_drive(),
                Some(BlockDeviceAction::UMOUNT) => self.umount_encrypted_drive(),
                None => Ok(()),
            },
            PasswordUsage::SUDOCOMMAND => Self::run_sudo_command(self),
        }
    }

    pub fn update_nvim_listen_address(&mut self) {
        if let Ok(nvim_listen_address) = std::env::var("NVIM_LISTEN_ADDRESS") {
            self.nvim_server = nvim_listen_address;
        } else if let Ok(nvim_listen_address) = Self::parse_nvim_address_from_ss_output() {
            self.nvim_server = nvim_listen_address;
        }
    }

    fn parse_nvim_address_from_ss_output() -> Result<String> {
        if !is_program_in_path(SS) {
            return Err(anyhow!("{SS} isn't installed"));
        }
        if let Ok(output) = std::process::Command::new(SS).arg("-l").output() {
            let output = String::from_utf8(output.stdout).unwrap_or_default();
            let content: String = output
                .split(&['\n', '\t', ' '])
                .filter(|w| w.contains(NVIM))
                .collect();
            if !content.is_empty() {
                return Ok(content);
            }
        }
        Err(anyhow!("Couldn't get nvim listen address from `ss` output"))
    }

    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    /// The action is only executed if the user typed the char `y`
    pub fn confirm(&mut self, c: char, confirmed_action: NeedConfirmation) -> Result<()> {
        if c == 'y' {
            let _ = self.match_confirmed_mode(confirmed_action);
        }
        self.selected().reset_edit_mode();
        self.selected().refresh_view()?;

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

    /// Select the left or right tab depending on where the user clicked.
    pub fn select_pane(&mut self, col: u16) -> Result<()> {
        let (width, _) = self.term_size()?;
        if self.dual_pane {
            if (col as usize) < width / 2 {
                self.select_tab(0)?;
            } else {
                self.select_tab(1)?;
            };
        } else {
            self.select_tab(0)?;
        }
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
