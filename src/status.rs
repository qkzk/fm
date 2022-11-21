use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{self, Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use skim::SkimItem;
use tuikit::term::Term;

use crate::args::Args;
use crate::bulkrename::Bulkrename;
use crate::color_cache::ColorCache;
use crate::config::Config;
use crate::copy_move::{copy, mover};
use crate::fileinfo::PathContent;
use crate::fm_error::{FmError, FmResult};
use crate::last_edition::LastEdition;
use crate::marks::Marks;
use crate::mode::{MarkAction, Mode};
use crate::tab::Tab;

#[derive(Clone)]
enum CutOrCopy {
    Cut,
    Copy,
}

pub struct Status {
    /// Vector of `Tab`, each of them are displayed in a separate tab.
    tabs: Vec<Tab>,
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
    /// Should it display the last selected file before exiting?
    pub print_path_on_quit: bool,
    /// terminal
    term: Arc<Term>,
}

impl Status {
    const MAX_TAB: u32 = 10;
    const MAX_PERMISSIONS: u32 = 0o777;

    pub fn new(args: Args, config: Config, height: usize, term: Arc<Term>) -> FmResult<Self> {
        Ok(Self {
            tabs: vec![Tab::new(args, config, height)?],
            index: 0,
            flagged: HashSet::new(),
            jump_index: 0,
            marks: Marks::read_from_config_file(),
            colors: ColorCache::default(),
            print_path_on_quit: false,
            term,
        })
    }

    pub fn len_index_of_tabs(&self) -> (usize, usize) {
        (self.tabs.len(), self.index)
    }

    pub fn new_tab(&mut self) {
        if self.tabs.len() < Self::MAX_TAB as usize {
            self.tabs.push(self.tabs[self.index].clone())
        }
    }

    pub fn go_tab(&mut self, digit: char) {
        let index = digit.to_digit(10).unwrap_or(Self::MAX_TAB) as usize;
        if self.tabs.len() > index {
            self.index = index
        }
    }

    pub fn drop_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.tabs.remove(self.index);
            if self.index > 0 {
                self.index = (self.index - 1) % self.len()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn next(&mut self) {
        if self.is_empty() {
            self.index = 0;
        } else {
            self.index = (self.index + 1) % self.len()
        }
    }

    pub fn prev(&mut self) {
        if self.is_empty() {
            self.index = 0
        } else if self.index > 0 {
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

    fn reset_statuses(&mut self) -> FmResult<()> {
        for status in self.tabs.iter_mut() {
            status.refresh_view()?
        }
        Ok(())
    }

    pub fn event_clear_flags(&mut self) -> FmResult<()> {
        self.flagged.clear();
        Ok(())
    }

    pub fn event_flag_all(&mut self) -> FmResult<()> {
        self.tabs[self.index]
            .path_content
            .files
            .iter()
            .for_each(|file| {
                self.flagged.insert(file.path.clone());
            });
        self.reset_statuses()
    }

    pub fn event_reverse_flags(&mut self) -> FmResult<()> {
        // for file in self.selected().path_content.files.iter() {
        //     self.toggle_flag_on_path(file.path.clone())
        // }

        self.tabs[self.index]
            .path_content
            .files
            .iter()
            .for_each(|file| {
                if self.flagged.contains(&file.path.clone()) {
                    self.flagged.remove(&file.path.clone());
                } else {
                    self.flagged.insert(file.path.clone());
                }
            });
        self.reset_statuses()
    }

    fn toggle_flag_on_path(&mut self, path: PathBuf) {
        if self.flagged.contains(&path) {
            self.flagged.remove(&path);
        } else {
            self.flagged.insert(path);
        };
    }

    pub fn create_tabs_from_skim(&mut self, output: Vec<Arc<dyn SkimItem>>) {
        for path in output.iter() {
            self.create_tab_from_skim_output(path)
        }
    }

    fn create_tab_from_skim_output(&mut self, cow_path: &Arc<dyn SkimItem>) {
        let mut status = self.selected().clone();
        let s_path = cow_path.output().to_string();
        if let Ok(path) = fs::canonicalize(path::Path::new(&s_path)) {
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    let _ = status.set_pathcontent(parent.to_path_buf());
                    self.tabs.push(status);
                }
            } else if path.is_dir() {
                let _ = status.set_pathcontent(path);
                self.tabs.push(status);
            }
        }
    }

    pub fn event_toggle_flag(&mut self) -> FmResult<()> {
        let file = self.tabs[self.index]
            .path_content
            .selected_file()
            .ok_or_else(|| FmError::new("No selected file"))?;
        self.toggle_flag_on_path(file.path.clone());
        self.selected().event_down_one_row();
        Ok(())
    }

    pub fn event_jumplist_next(&mut self) {
        if self.jump_index < self.flagged.len() {
            self.jump_index += 1;
        }
    }

    pub fn event_jumplist_prev(&mut self) {
        if self.jump_index > 0 {
            self.jump_index -= 1;
        }
    }

    pub fn event_chmod(&mut self) -> FmResult<()> {
        if self.selected().path_content.files.is_empty() {
            return Ok(());
        }
        self.selected().mode = Mode::Chmod;
        if self.flagged.is_empty() {
            self.flagged.insert(
                self.tabs[self.index]
                    .path_content
                    .selected_file()
                    .unwrap()
                    .path
                    .clone(),
            );
        };
        self.reset_statuses()
    }

    pub fn event_jump(&mut self) {
        if !self.flagged.is_empty() {
            self.jump_index = 0;
            self.selected().mode = Mode::Jump
        }
    }

    pub fn event_marks_new(&mut self) {
        self.selected().mode = Mode::Marks(MarkAction::New)
    }

    pub fn event_marks_jump(&mut self) {
        self.selected().mode = Mode::Marks(MarkAction::Jump)
    }

    pub fn exec_marks_new(&mut self, c: char) -> FmResult<()> {
        let path = self.selected().path_content.path.clone();
        self.marks.new_mark(c, path)?;
        self.selected().event_normal()
    }

    pub fn exec_marks_jump(&mut self, c: char) -> FmResult<()> {
        if let Some(path) = self.marks.get(c) {
            let path = path.to_owned();
            self.selected().history.push(&path);
            self.selected().path_content = PathContent::new(path, self.selected().show_hidden)?;
        };
        self.selected().event_normal()
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn event_symlink(&mut self) -> FmResult<()> {
        for oldpath in self.flagged.iter() {
            let newpath = self.tabs[self.index].path_content.path.clone().join(
                oldpath
                    .as_path()
                    .file_name()
                    .ok_or_else(|| FmError::new("File not found"))?,
            );
            std::os::unix::fs::symlink(oldpath, newpath)?;
        }
        self.clear_flags_and_reset_view()
    }

    pub fn event_bulkrename(&mut self) -> FmResult<()> {
        Bulkrename::new(self.filtered_flagged_files())?.rename(&self.selected_non_mut().opener)?;
        self.selected().refresh_view()
    }

    fn filtered_flagged_files(&self) -> Vec<&Path> {
        let path_content = self.selected_non_mut().path_content.clone();
        self.flagged
            .iter()
            .filter(|p| path_content.contains(p))
            .map(|p| p.as_path())
            .collect()
    }

    fn cut_or_copy_flagged_files(&mut self, cut_or_copy: CutOrCopy) -> FmResult<()> {
        let sources: Vec<PathBuf> = self.flagged.iter().map(|path| path.to_owned()).collect();
        let dest = self
            .selected_non_mut()
            .path_str()
            .ok_or_else(|| FmError::new("unreadable path"))?;
        if let CutOrCopy::Cut = cut_or_copy {
            mover(sources, dest, self.term.clone())?
        } else {
            copy(sources, dest, self.term.clone())?
        }
        self.clear_flags_and_reset_view()
    }

    fn clear_flags_and_reset_view(&mut self) -> FmResult<()> {
        self.flagged.clear();
        self.selected().path_content.reset_files()?;
        let len = self.tabs[self.index].path_content.files.len();
        self.selected().window.reset(len);
        self.reset_statuses()
    }

    fn exec_copy_paste(&mut self) -> FmResult<()> {
        self.cut_or_copy_flagged_files(CutOrCopy::Copy)
    }

    fn exec_cut_paste(&mut self) -> FmResult<()> {
        self.cut_or_copy_flagged_files(CutOrCopy::Cut)
    }

    fn exec_delete_files(&mut self) -> FmResult<()> {
        for pathbuf in self.flagged.iter() {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(pathbuf)?;
            } else {
                std::fs::remove_file(pathbuf)?;
            }
        }
        self.clear_flags_and_reset_view()
    }

    pub fn exec_chmod(&mut self) -> FmResult<()> {
        if self.selected().input.string.is_empty() {
            return Ok(());
        }
        let permissions: u32 =
            u32::from_str_radix(&self.selected().input.string, 8).unwrap_or(0_u32);
        if permissions <= Self::MAX_PERMISSIONS {
            for path in self.flagged.iter() {
                Self::set_permissions(path.clone(), permissions)?
            }
            self.flagged.clear()
        }
        self.selected().refresh_view()?;
        self.reset_statuses()
    }

    pub fn exec_jump(&mut self) -> FmResult<()> {
        self.selected().input.string.clear();
        let jump_list: Vec<&PathBuf> = self.flagged.iter().collect();
        let jump_target = jump_list[self.jump_index].clone();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent.to_path_buf(),
            None => jump_target.clone(),
        };
        self.selected().history.push(&target_dir);
        self.selected().path_content = PathContent::new(target_dir, self.selected().show_hidden)?;
        if let Some(index) = self.find_jump_target(&jump_target) {
            self.selected().line_index = index;
        } else {
            self.selected().line_index = 0;
        }

        let s_index = self.tabs[self.index].line_index;
        self.tabs[self.index].path_content.select_index(s_index);
        let len = self.tabs[self.index].path_content.files.len();
        self.selected().window.reset(len);
        self.selected().window.scroll_to(s_index);
        Ok(())
    }

    fn find_jump_target(&mut self, jump_target: &Path) -> Option<usize> {
        self.selected()
            .path_content
            .files
            .iter()
            .position(|file| file.path == jump_target)
    }

    pub fn exec_last_edition(&mut self) -> FmResult<()> {
        self._exec_last_edition()?;
        self.selected().mode = Mode::Normal;
        self.selected().last_edition = LastEdition::Nothing;
        Ok(())
    }

    fn _exec_last_edition(&mut self) -> FmResult<()> {
        match self.selected().last_edition {
            LastEdition::Delete => self.exec_delete_files(),
            LastEdition::CutPaste => self.exec_cut_paste(),
            LastEdition::CopyPaste => self.exec_copy_paste(),
            LastEdition::Nothing => Ok(()),
        }
    }

    fn set_permissions(path: PathBuf, permissions: u32) -> FmResult<()> {
        Ok(std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(permissions),
        )?)
    }

    pub fn exec_regex(&mut self) -> Result<(), regex::Error> {
        if !self.selected().input.string.is_empty() {
            self.flagged.clear();
            let re = Regex::new(&self.selected().input.string)?;
            for file in self.tabs[self.index].path_content.files.iter() {
                if re.is_match(&file.path.to_string_lossy()) {
                    self.flagged.insert(file.path.clone());
                }
            }
        }
        self.selected().input.reset();
        Ok(())
    }
}
