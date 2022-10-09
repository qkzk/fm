use regex::Regex;
use std::collections::HashSet;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::args::Args;
use crate::config::Config;
use crate::content_window::ContentWindow;
use crate::fileinfo::PathContent;
use crate::last_edition::LastEdition;
use crate::mode::Mode;
use crate::status::Status;

pub struct Tabs {
    pub statuses: Vec<Status>,
    pub index: usize,
    /// String typed by the user in relevant modes
    pub flagged: HashSet<PathBuf>,
}

impl Tabs {
    const MAX_PERMISSIONS: u32 = 0o777;

    pub fn new(args: Args, config: Config, height: usize) -> Self {
        Self {
            statuses: vec![Status::new(args, config, height)],
            index: 0,
            flagged: HashSet::new(),
        }
    }

    pub fn new_tab(&mut self) {
        self.statuses.push(self.statuses[self.index].clone())
    }

    pub fn drop_tab(&mut self) {
        if self.statuses.len() > 1 {
            self.statuses.remove(self.index);
            if self.index > 0 {
                self.index = (self.index - 1) % self.len()
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.statuses.is_empty()
    }

    pub fn len(&self) -> usize {
        self.statuses.len()
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

    pub fn selected(&mut self) -> &mut Status {
        &mut self.statuses[self.index]
    }

    pub fn selected_non_mut(&self) -> &Status {
        &self.statuses[self.index]
    }

    fn reset_statuses(&mut self) {
        for status in self.statuses.iter_mut() {
            status.refresh_view()
        }
    }

    pub fn event_clear_flags(&mut self) {
        self.flagged.clear();
        self.reset_statuses()
    }

    pub fn event_flag_all(&mut self) {
        self.statuses[self.index]
            .path_content
            .files
            .iter()
            .for_each(|file| {
                self.flagged.insert(file.path.clone());
            });
        self.reset_statuses()
    }

    pub fn event_reverse_flags(&mut self) {
        // TODO: is there a way to use `toggle_flag_on_path` ? 2 mutable borrows...
        self.statuses[self.index]
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
        self.reset_statuses()
    }

    pub fn event_toggle_flag(&mut self) {
        if self.selected().path_content.files.is_empty() {
            return;
        }
        self.toggle_flag_on_path(
            self.statuses[self.index]
                .path_content
                .selected_file()
                .unwrap()
                .path
                .clone(),
        );
        if self.selected().line_index
            < self.selected().path_content.files.len() - ContentWindow::WINDOW_MARGIN_TOP
        {
            self.selected().line_index += 1
        }
        self.selected().path_content.select_next();
        let dest = self.statuses[self.index].line_index;
        self.statuses[self.index].window.scroll_down_one(dest)
    }

    pub fn event_jumplist_next(&mut self) {
        if self.selected().jump_index < self.flagged.len() {
            self.selected().jump_index += 1;
        }
    }

    pub fn event_jumplist_prev(&mut self) {
        if self.selected().jump_index > 0 {
            self.selected().jump_index -= 1;
        }
    }

    pub fn event_chmod(&mut self) {
        if self.selected().path_content.files.is_empty() {
            return;
        }
        self.selected().mode = Mode::Chmod;
        if self.flagged.is_empty() {
            self.flagged.insert(
                self.statuses[self.index]
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
            self.selected().jump_index = 0;
            self.selected().mode = Mode::Jump
        }
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn event_symlink(&mut self) {
        self.flagged.iter().for_each(|oldpath| {
            let newpath = self.statuses[self.index]
                .path_content
                .path
                .clone()
                .join(oldpath.as_path().file_name().unwrap());
            std::os::unix::fs::symlink(oldpath, newpath).unwrap_or(());
        });
        self.flagged.clear();
        self.selected().path_content.reset_files();
        let len = self.statuses[self.index].path_content.files.len();
        self.statuses[self.index].window.reset(len);
        self.reset_statuses()
    }

    fn exec_copy_paste(&mut self) {
        self.flagged.iter().for_each(|oldpath| {
            let newpath = self.statuses[self.index]
                .path_content
                .path
                .clone()
                .join(oldpath.as_path().file_name().unwrap());
            std::fs::copy(oldpath, newpath).unwrap_or(0);
        });
        self.flagged.clear();
        self.selected().path_content.reset_files();
        let len = self.statuses[self.index].path_content.files.len();
        self.selected().window.reset(len);
        self.reset_statuses()
    }

    fn exec_cut_paste(&mut self) {
        self.flagged.iter().for_each(|oldpath| {
            let newpath = self.statuses[self.index]
                .path_content
                .path
                .clone()
                .join(oldpath.as_path().file_name().unwrap());
            std::fs::rename(oldpath, newpath).unwrap_or(());
        });
        self.flagged.clear();
        self.selected().path_content.reset_files();
        let len = self.statuses[self.index].path_content.files.len();
        self.selected().window.reset(len);
        self.reset_statuses()
    }

    fn exec_delete_files(&mut self) {
        self.flagged.iter().for_each(|pathbuf| {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(pathbuf).unwrap_or(());
            } else {
                std::fs::remove_file(pathbuf).unwrap_or(());
            }
        });
        self.flagged.clear();
        self.selected().path_content.reset_files();
        let len = self.statuses[self.index].path_content.files.len();
        self.selected().window.reset(len);
        self.reset_statuses()
    }

    pub fn exec_chmod(&mut self) {
        if self.selected().input.string.is_empty() {
            return;
        }
        let permissions: u32 =
            u32::from_str_radix(&self.selected().input.string, 8).unwrap_or(0_u32);
        if permissions <= Self::MAX_PERMISSIONS {
            for path in self.flagged.iter() {
                Self::set_permissions(path.clone(), permissions).unwrap_or(())
            }
            self.flagged.clear()
        }
        self.selected().input.string.clear();
        self.selected().refresh_view();
        self.reset_statuses()
    }

    pub fn exec_jump(&mut self) {
        self.selected().input.reset();
        let jump_list: Vec<&PathBuf> = self.flagged.iter().collect();
        let jump_target = jump_list[self.statuses[self.index].jump_index].clone();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent.to_path_buf(),
            None => jump_target.clone(),
        };
        self.selected().history.push(&target_dir);
        self.selected().path_content = PathContent::new(target_dir, self.selected().show_hidden);
        self.selected().line_index = self
            .selected()
            .path_content
            .files
            .iter()
            .position(|file| file.path == jump_target.clone())
            .unwrap_or(0);
        let s_index = self.statuses[self.index].line_index;
        self.statuses[self.index].path_content.select_index(s_index);
        let len = self.statuses[self.index].path_content.files.len();
        self.selected().window.reset(len);
        self.selected().window.scroll_to(s_index);
    }

    pub fn exec_last_edition(&mut self) {
        match self.selected().last_edition {
            LastEdition::Delete => self.exec_delete_files(),
            LastEdition::CutPaste => self.exec_cut_paste(),
            LastEdition::CopyPaste => self.exec_copy_paste(),
            LastEdition::Nothing => (),
        }
        self.selected().mode = Mode::Normal;
        self.selected().last_edition = LastEdition::Nothing;
    }

    fn set_permissions(path: PathBuf, permissions: u32) -> Result<(), std::io::Error> {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(permissions))
    }

    pub fn exec_regex(&mut self) {
        let re = Regex::new(&self.selected().input.string).unwrap();
        if !self.selected().input.string.is_empty() {
            self.flagged.clear();
            for file in self.statuses[self.index].path_content.files.iter() {
                if re.is_match(file.path.to_str().unwrap()) {
                    self.flagged.insert(file.path.clone());
                }
            }
        }
        self.selected().input.reset();
    }
}
