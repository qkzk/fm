use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use skim::SkimItem;
use sysinfo::{Disk, DiskExt, System, SystemExt};
use tuikit::term::Term;

use crate::args::Args;
use crate::color_cache::ColorCache;
use crate::config::Config;
use crate::copy_move::{copy_move, CopyMove};
use crate::fm_error::{FmError, FmResult};
use crate::marks::Marks;
use crate::skim::Skimer;
use crate::tab::Tab;
use crate::utils::disk_space;

pub struct Status {
    /// Vector of `Tab`, each of them are displayed in a separate tab.
    pub tabs: [Tab; 2],
    /// Index of the current selected tab
    pub index: usize,
    /// Set of flagged files
    pub flagged: HashSet<PathBuf>,
    /// Index in the jump list
    pub jump_index: usize,
    /// Marks allows you to jump to a save mark
    pub marks: Marks,
    /// Colors for extension
    pub colors: ColorCache,
    /// terminal
    term: Arc<Term>,
    skimer: Skimer,
    dual_pane: bool,
    sys: System,
    pub display_full: bool,
}

impl Status {
    pub const MAX_PERMISSIONS: u32 = 0o777;

    pub fn new(
        args: Args,
        config: Config,
        height: usize,
        term: Arc<Term>,
        help: String,
    ) -> FmResult<Self> {
        let mut tab = Tab::new(args, config, height, help)?;
        let sys = System::new_all();
        tab.shortcut
            .update_mount_points(Self::disks_mounts(sys.disks()));

        Ok(Self {
            tabs: [tab.clone(), tab],
            index: 0,
            flagged: HashSet::new(),
            jump_index: 0,
            marks: Marks::read_from_config_file(),
            colors: ColorCache::default(),
            skimer: Skimer::new(term.clone()),
            term,
            dual_pane: true,
            sys,
            display_full: true,
        })
    }

    fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn next(&mut self) {
        if !self.dual_pane {
            return;
        }

        self.index = (self.index + 1) % self.len()
    }

    pub fn prev(&mut self) {
        if !self.dual_pane {
            return;
        }
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.len() - 1
        }
    }

    pub fn selected(&mut self) -> &mut Tab {
        &mut self.tabs[self.index]
    }

    pub fn selected_non_mut(&self) -> &Tab {
        &self.tabs[self.index]
    }

    pub fn reset_statuses(&mut self) -> FmResult<()> {
        for status in self.tabs.iter_mut() {
            status.refresh_view()?
        }
        Ok(())
    }

    pub fn toggle_flag_on_path(&mut self, path: PathBuf) {
        if self.flagged.contains(&path) {
            self.flagged.remove(&path);
        } else {
            self.flagged.insert(path);
        };
    }

    pub fn create_tabs_from_skim(&mut self) -> FmResult<()> {
        for path in self
            .skimer
            .no_source(
                self.selected_non_mut()
                    .path_str()
                    .ok_or_else(|| FmError::new("skim error"))?,
            )
            .iter()
        {
            self.create_tab_from_skim_output(path)
        }
        Ok(())
    }

    fn create_tab_from_skim_output(&mut self, cow_path: &Arc<dyn SkimItem>) {
        let mut tab = self.selected().clone();
        let s_path = cow_path.output().to_string();
        if let Ok(path) = fs::canonicalize(path::Path::new(&s_path)) {
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    let _ = tab.set_pathcontent(parent.to_path_buf());
                }
            } else if path.is_dir() {
                let _ = tab.set_pathcontent(path);
                self.tabs[self.index] = tab;
            }
        }
    }

    pub fn filtered_flagged_files(&self) -> Vec<&Path> {
        let path_content = self.selected_non_mut().path_content.clone();
        self.flagged
            .iter()
            .filter(|p| path_content.contains(p))
            .map(|p| p.as_path())
            .collect()
    }

    pub fn cut_or_copy_flagged_files(&mut self, cut_or_copy: CopyMove) -> FmResult<()> {
        let sources: Vec<PathBuf> = self.flagged.iter().map(|path| path.to_owned()).collect();
        let dest = self
            .selected_non_mut()
            .path_str()
            .ok_or_else(|| FmError::new("unreadable path"))?;
        copy_move(cut_or_copy, sources, dest, self.term.clone())?;
        self.clear_flags_and_reset_view()
    }

    pub fn clear_flags_and_reset_view(&mut self) -> FmResult<()> {
        self.flagged.clear();
        self.selected().path_content.reset_files()?;
        let len = self.tabs[self.index].path_content.files.len();
        self.selected().window.reset(len);
        self.reset_statuses()
    }

    pub fn find_jump_target(&mut self, jump_target: &Path) -> Option<usize> {
        self.selected()
            .path_content
            .files
            .iter()
            .position(|file| file.path == jump_target)
    }

    pub fn set_permissions(path: PathBuf, permissions: u32) -> FmResult<()> {
        Ok(std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(permissions),
        )?)
    }

    pub fn select_from_regex(&mut self) -> Result<(), regex::Error> {
        if self.selected().input.string.is_empty() {
            return Ok(());
        }
        self.flagged.clear();
        let re = Regex::new(&self.selected().input.string)?;
        for file in self.tabs[self.index].path_content.files.iter() {
            if re.is_match(&file.path.to_string_lossy()) {
                self.flagged.insert(file.path.clone());
            }
        }
        Ok(())
    }

    pub fn select_tab(&mut self, index: usize) -> FmResult<()> {
        if index >= self.tabs.len() {
            Err(FmError::new(&format!(
                "Only {} tabs. Can't select tab {}",
                self.tabs.len(),
                index
            )))
        } else {
            self.index = index;
            Ok(())
        }
    }

    pub fn set_dual_pane(&mut self, dual_pane: bool) {
        self.dual_pane = dual_pane;
    }

    pub fn refresh_disks(&mut self) {
        self.sys.refresh_disks_list();
        let disks = self.sys.disks();
        self.tabs[0]
            .shortcut
            .update_mount_points(Self::disks_mounts(disks));
        self.tabs[1]
            .shortcut
            .update_mount_points(Self::disks_mounts(disks));
    }

    pub fn disks(&self) -> &[Disk] {
        self.sys.disks()
    }

    pub fn disk_spaces(&self) -> (String, String) {
        let disks = self.disks();
        (
            disk_space(disks, &self.tabs[0].path_content.path),
            disk_space(disks, &self.tabs[1].path_content.path),
        )
    }

    pub fn disks_mounts(disks: &[Disk]) -> Vec<&Path> {
        disks.iter().map(|d| d.mount_point()).collect()
    }

    pub fn term_size(&self) -> FmResult<(usize, usize)> {
        Ok(self.term.term_size()?)
    }
}
