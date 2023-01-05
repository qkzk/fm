use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use regex::Regex;
use skim::SkimItem;
use sysinfo::{Disk, DiskExt, System, SystemExt};
use tuikit::term::Term;
use users::UsersCache;

use crate::args::Args;
use crate::config::{Colors, Config};
use crate::constant_strings_paths::OPENER_PATH;
use crate::copy_move::{copy_move, CopyMove};
use crate::flagged::Flagged;
use crate::fm_error::{FmError, FmResult};
use crate::marks::Marks;
use crate::opener::{load_opener, Opener};
use crate::preview::Directory;
use crate::skim::Skimer;
use crate::tab::Tab;
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
    /// The opener used by the application.
    pub opener: Opener,
    /// The help string.
    pub help: String,
    /// The trash
    pub trash: Trash,
    pub config_colors: Colors,
}

impl Status {
    /// Max valid permission number, ie `0o777`.
    pub const MAX_PERMISSIONS: u32 = 0o777;

    /// Creates a new status for the application.
    /// It requires most of the information (arguments, configuration, height
    /// of the terminal, the formated help string).
    pub fn new(
        args: Args,
        config: Config,
        height: usize,
        term: Arc<Term>,
        help: String,
    ) -> FmResult<Self> {
        // unsafe because of UsersCache::with_all_users
        let terminal = config.terminal();
        let sys = System::new_all();
        let opener = load_opener(OPENER_PATH, terminal).unwrap_or_else(|_| Opener::new(terminal));
        let users_cache = unsafe { Rc::new(UsersCache::with_all_users()) };
        let mut tab = Tab::new(args, height, users_cache)?;
        tab.shortcut
            .extend_with_mount_points(&Self::disks_mounts(sys.disks()));
        let trash = Trash::new()?;

        Ok(Self {
            tabs: [tab.clone(), tab],
            index: 0,
            flagged: Flagged::default(),
            marks: Marks::read_from_config_file(),
            // colors: ColorCache::default(),
            config_colors: config.colors,
            skimer: Skimer::new(term.clone()),
            term,
            dual_pane: true,
            system_info: sys,
            display_full: true,
            opener,
            help,
            trash,
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
    pub fn reset_tabs_view(&mut self) -> FmResult<()> {
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
    pub fn skim_output_to_tab(&mut self) -> FmResult<()> {
        if let Some(skim_output) = self
            .skimer
            .no_source(
                self.selected_non_mut()
                    .selected()
                    .ok_or_else(|| FmError::custom("skim", "no selected file"))?
                    .path
                    .to_str()
                    .ok_or_else(|| FmError::custom("skim", "skim error"))?,
            )
            .first()
        {
            self._update_tab_from_skim_output(skim_output)?;
        }
        Ok(())
    }

    fn _update_tab_from_skim_output(&mut self, skim_outut: &Arc<dyn SkimItem>) -> FmResult<()> {
        let path = fs::canonicalize(skim_outut.output().to_string())?;
        let tab = self.selected();
        if path.is_file() {
            if let Some(parent) = path.parent() {
                tab.set_pathcontent(parent)?;
                let filename = filename_from_path(&path)?;
                tab.search_from(filename, 0);
            }
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
    pub fn cut_or_copy_flagged_files(&mut self, cut_or_copy: CopyMove) -> FmResult<()> {
        let sources = self.flagged.content.clone();
        let dest = self
            .selected_non_mut()
            .path_content_str()
            .ok_or_else(|| FmError::custom("cut or copy", "unreadable path"))?;
        copy_move(cut_or_copy, sources, dest, self.term.clone())?;
        self.clear_flags_and_reset_view()
    }

    /// Empty the flagged files, reset the view of every tab.
    pub fn clear_flags_and_reset_view(&mut self) -> FmResult<()> {
        self.flagged.clear();
        self.reset_tabs_view()
    }

    /// Set the permissions of the flagged files according to a given permission.
    /// If the permission are invalid or if the user can't edit them, it may fail.
    pub fn set_permissions<P>(path: P, permissions: u32) -> FmResult<()>
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
    pub fn select_tab(&mut self, index: usize) -> FmResult<()> {
        if index >= self.tabs.len() {
            Err(FmError::custom(
                "select tab",
                &format!("Only {} tabs. Can't select tab {}", self.tabs.len(), index),
            ))
        } else {
            self.index = index;
            Ok(())
        }
    }

    /// Set dual pane mode to true or false.
    pub fn set_dual_pane(&mut self, dual_pane: bool) {
        self.dual_pane = dual_pane;
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
    pub fn term_size(&self) -> FmResult<(usize, usize)> {
        Ok(self.term.term_size()?)
    }

    /// Returns a string representing the current path in the selected tab.
    pub fn selected_path_str(&self) -> &str {
        self.selected_non_mut()
            .path_content_str()
            .unwrap_or_default()
    }

    /// Refresh the existing users.
    pub fn refresh_users(&mut self) -> FmResult<()> {
        let users_cache = Rc::new(unsafe { UsersCache::with_all_users() });
        for tab in self.tabs.iter_mut() {
            tab.refresh_users(users_cache.clone())?;
        }
        Ok(())
    }

    pub fn remove_tree(&mut self) -> FmResult<()> {
        let path = self.selected_non_mut().path_content.path.clone();
        let users_cache = &self.selected_non_mut().path_content.users_cache;
        self.selected().directory = Directory::empty(&path, users_cache)?;
        Ok(())
    }
}
