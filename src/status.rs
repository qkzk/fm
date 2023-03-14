use std::borrow::BorrowMut;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use log::info;
use regex::Regex;
use skim::SkimItem;
use sysinfo::{Disk, DiskExt, RefreshKind, System, SystemExt};
use tuikit::prelude::{from_keyname, Event};
use tuikit::term::Term;
use users::UsersCache;

use crate::args::Args;
use crate::bulkrename::Bulk;
use crate::compress::Compresser;
use crate::config::Colors;
use crate::constant_strings_paths::{OPENER_PATH, TUIS_PATH};
use crate::copy_move::{copy_move, CopyMove};
use crate::cryptsetup::CryptoDeviceOpener;
use crate::flagged::Flagged;
use crate::iso::IsoMounter;
// use crate::keybindings::to_keyname;
use crate::marks::Marks;
use crate::opener::{load_opener, Opener};
use crate::preview::{Directory, Preview};
use crate::shell_menu::{load_shell_menu, ShellMenu};
use crate::skim::Skimer;
use crate::tab::Tab;
use crate::term_manager::MIN_WIDTH_FOR_DUAL_PANE;
use crate::trash::Trash;
use crate::utils::{disk_space, filename_from_path};

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
    /// Colors for extension
    // pub colors: ColorCache,
    /// terminal
    term: Arc<Term>,
    skimer: Skimer,
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
    pub iso_mounter: Option<IsoMounter>,
    /// Compression methods
    pub compression: Compresser,
    /// NVIM RPC server address
    pub nvim_server: String,
    pub force_clear: bool,
    pub bulk: Bulk,
    pub shell_menu: ShellMenu,
}

impl Status {
    /// Max valid permission number, ie `0o777`.
    pub const MAX_PERMISSIONS: u32 = 0o777;

    /// Creates a new status for the application.
    /// It requires most of the information (arguments, configuration, height
    /// of the terminal, the formated help string).
    pub fn new(
        args: Args,
        height: usize,
        term: Arc<Term>,
        help: String,
        terminal: &str,
    ) -> Result<Self> {
        let opener = load_opener(OPENER_PATH, terminal).unwrap_or_else(|_| {
            eprintln!("Couldn't read the opener config file at {OPENER_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/opener.yaml for an example. Using default.");
            info!("Couldn't read opener file at {OPENER_PATH}. Using default.");
            Opener::new(terminal)
        });
        let Ok(shell_menu) = load_shell_menu(TUIS_PATH) else {
            eprintln!("Couldn't load the TUIs config file at {TUIS_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/tuis.yaml for an example"); 
            info!("Couldn't read tuis file at {TUIS_PATH}. Exiting");
            std::process::exit(1);
        };

        let sys = System::new_with_specifics(RefreshKind::new().with_disks());
        let nvim_server = args.server.clone();
        let encrypted_devices = CryptoDeviceOpener::default();
        let trash = Trash::new()?;
        let compression = Compresser::default();
        let force_clear = false;
        let bulk = Bulk::default();

        // unsafe because of UsersCache::with_all_users
        let users_cache = unsafe { UsersCache::with_all_users() };
        let mut right_tab = Tab::new(args.clone(), height, users_cache)?;
        right_tab
            .shortcut
            .extend_with_mount_points(&Self::disks_mounts(sys.disks()));

        // unsafe because of UsersCache::with_all_users
        let users_cache2 = unsafe { UsersCache::with_all_users() };
        let mut left_tab = Tab::new(args, height, users_cache2)?;
        left_tab
            .shortcut
            .extend_with_mount_points(&Self::disks_mounts(sys.disks()));
        let iso_mounter = None;

        Ok(Self {
            tabs: [left_tab, right_tab],
            index: 0,
            flagged: Flagged::default(),
            marks: Marks::read_from_config_file(),
            skimer: Skimer::new(term.clone()),
            term,
            dual_pane: true,
            preview_second: false,
            system_info: sys,
            display_full: true,
            opener,
            help,
            trash,
            encrypted_devices,
            compression,
            nvim_server,
            force_clear,
            bulk,
            shell_menu,
            iso_mounter,
        })
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

    /// Replace the tab content with the first result of skim.
    /// It calls skim, reads its output, then update the tab content.
    pub fn skim_output_to_tab(&mut self) -> Result<()> {
        let skim = self.skimer.search_filename(
            self.selected_non_mut()
                .selected()
                .context("skim: no selected file")?
                .path
                .to_str()
                .context("skim error")?,
        );
        let Some(output) = skim.first() else {return Ok(())};
        self._update_tab_from_skim_output(output)
    }

    /// Replace the tab content with the first result of skim.
    /// It calls skim, reads its output, then update the tab content.
    /// The output is splited at `:` since we only care about the path, not the line number.
    pub fn skim_line_output_to_tab(&mut self) -> Result<()> {
        let skim = self.skimer.search_line_in_file();
        let Some(output) = skim.first() else {return Ok(())};
        self._update_tab_from_skim_line_output(output)
    }

    /// Run a command directly from help.
    /// Search a command in skim, if it's a keybinding, run it directly.
    /// If the result can't be parsed, nothing is done.
    pub fn skim_find_keybinding(&mut self) -> Result<()> {
        let skim = self.skimer.search_in_text(self.help.clone());
        let Some(output) = skim.first() else { return Ok(()) };
        let line = output.output().into_owned();
        let Some(keybind) = line.split(':').next() else { return Ok(()) };
        let Some(keyname) = parse_keyname(keybind) else { return Ok(()) };
        let Some(key) = from_keyname(&keyname) else { return Ok(()) };
        let event = Event::Key(key);
        let _ = self.term.borrow_mut().send_event(event);
        Ok(())
    }

    fn _update_tab_from_skim_line_output(&mut self, skim_output: &Arc<dyn SkimItem>) -> Result<()> {
        let output_str = skim_output.output().to_string();
        let Some(filename) = output_str.split(':').next() else { return Ok(());};
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
            let Some(parent) = path.parent() else { return Ok(()) };
            tab.set_pathcontent(parent)?;
            let filename = filename_from_path(&path)?;
            tab.search_from(filename, 0);
        } else if path.is_dir() {
            tab.set_pathcontent(&path)?;
        }

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
        let dest = self
            .selected_non_mut()
            .path_content_str()
            .context("cut or copy: unreadable path")?;
        copy_move(cut_or_copy, sources, dest, self.term.clone())?;
        self.clear_flags_and_reset_view()
    }

    /// Empty the flagged files, reset the view of every tab.
    pub fn clear_flags_and_reset_view(&mut self) -> Result<()> {
        self.flagged.clear();
        self.reset_tabs_view()
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
        for tab in self.tabs.iter_mut() {
            let users_cache = unsafe { UsersCache::with_all_users() };
            tab.refresh_users(users_cache)?;
        }
        Ok(())
    }

    /// Drop the current tree, replace it with an empty one.
    pub fn remove_tree(&mut self) -> Result<()> {
        let path = self.selected_non_mut().path_content.path.clone();
        let users_cache = &self.selected_non_mut().path_content.users_cache;
        self.selected().directory = Directory::empty(&path, users_cache)?;
        Ok(())
    }

    /// Updates the encrypted devices
    pub fn read_encrypted_devices(&mut self) -> Result<()> {
        self.encrypted_devices.update()?;
        Ok(())
    }

    /// Force a preview on the second pane
    pub fn force_preview(&mut self, colors: &Colors) -> Result<()> {
        let fileinfo = &self.tabs[0]
            .selected()
            .context("force preview: No file to select")?;
        let users_cache = &self.tabs[0].path_content.users_cache;
        self.tabs[0].preview =
            Preview::new(fileinfo, users_cache, self, colors).unwrap_or_default();
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
}

fn parse_keyname(keyname: &str) -> Option<String> {
    let mut split = keyname.split('(');
    let Some(mutator) = split.next() else { return None; };
    let mut mutator = mutator.to_lowercase();
    let Some(param) = split.next() else { return Some(mutator) };
    let mut param = param.to_owned();
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
