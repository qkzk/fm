use std::borrow::Borrow;
use std::cmp::min;
use std::path;

use anyhow::{Context, Result};

use crate::common::{
    has_last_modification_happened_less_than, path_to_string, row_to_window_index,
};
use crate::io::Args;
use crate::modes::Content;
use crate::modes::Directory;
use crate::modes::FileInfo;
use crate::modes::FilterKind;
use crate::modes::History;
use crate::modes::Preview;
use crate::modes::Selectable;
use crate::modes::SortKind;
use crate::modes::Users;
use crate::modes::{ContentWindow, FileKind};
use crate::modes::{Display, Edit};
use crate::modes::{Go, To, Tree};

pub struct TabSettings {
    /// read from command line
    pub show_hidden: bool,
    /// The filter use before displaying files
    pub filter: FilterKind,
    /// The kind of sort used to display the files.
    pub sort_kind: SortKind,
}

impl TabSettings {
    fn new(args: &Args) -> Self {
        let filter = FilterKind::All;
        let show_hidden = args.all;
        let sort_kind = SortKind::default();
        Self {
            show_hidden,
            filter,
            sort_kind,
        }
    }

    fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
    }

    /// Apply the filter.
    pub fn set_filter(&mut self, filter: FilterKind) {
        self.filter = filter
    }

    /// Update the kind of sort from a char typed by the user.
    fn update_sort_from_char(&mut self, c: char) {
        self.sort_kind.update_from_char(c)
    }
}

/// Holds every thing about the current tab of the application.
/// Most of the mutation is done externally.
pub struct Tab {
    /// Kind of display: `Preview, Normal, Tree`
    pub display_mode: Display,

    /// Files in current path
    pub directory: Directory,
    /// Tree representation of the same path
    pub tree: Tree,
    /// Lines of the previewed files.
    /// Empty if not in preview mode.
    pub preview: Preview,

    /// The edit mode the application is currenty in.
    /// Most of the time is spent in `EditMode::Nothing`
    pub edit_mode: Edit,

    /// The indexes of displayed file
    pub window: ContentWindow,
    /// Height of the terminal window
    pub height: usize,

    /// Internal & display settings:
    /// show hidden files ?
    /// sort method
    /// filter kind
    pub settings: TabSettings,

    /// Last searched string
    pub searched: Option<String>,
    /// Visited directories
    pub history: History,
    /// Users & groups
    pub users: Users,
}

impl Tab {
    /// Creates a new tab from args and height.
    ///
    /// # Errors
    ///
    /// It reads a path from args, which is defaulted to the starting path.
    /// It explores the path and creates a content.
    /// The path is then selected. If no path was provide from args, the current folder `.` is selected.
    /// Every other attribute has its default value.
    ///
    /// # Errors
    ///
    /// it may fail if the path:
    /// - doesn't exist
    /// - can't be explored
    /// - has no parent and isn't a directory (which can't happen)
    pub fn new(args: &Args, height: usize, users: Users) -> Result<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let start_dir = if path.is_dir() {
            &path
        } else {
            path.parent().context("")?
        };
        let settings = TabSettings::new(args);
        let mut directory =
            Directory::new(start_dir, &users, &settings.filter, settings.show_hidden)?;
        let display_mode = Display::default();
        let edit_mode = Edit::Nothing;
        let mut window = ContentWindow::new(directory.content.len(), height);
        let preview = Preview::Empty;
        let history = History::default();
        let searched = None;
        let index = directory.select_file(&path);
        let tree = Tree::default();

        window.scroll_to(index);
        Ok(Self {
            display_mode,
            edit_mode,
            window,
            directory,
            height,
            preview,
            searched,
            history,
            users,
            tree,
            settings,
        })
    }

    /// Returns the directory owning the selected file.
    /// In Tree mode, it's the current directory if the selected node is a directory,
    /// its parent otherwise.
    /// In normal mode it's the current working directory.
    pub fn directory_of_selected(&self) -> Result<&path::Path> {
        match self.display_mode {
            Display::Tree => self.tree.directory_of_selected().context("No parent"),
            _ => Ok(&self.directory.path),
        }
    }

    /// Current path of this tab.
    pub fn current_path(&self) -> &path::Path {
        self.directory.path.borrow()
    }

    /// Fileinfo of the selected element.
    pub fn current_file(&self) -> Result<FileInfo> {
        match self.display_mode {
            Display::Tree => {
                let node = self.tree.selected_node().context("no selected node")?;
                node.fileinfo(&self.users)
            }
            _ => Ok(self
                .directory
                .selected()
                .context("no selected file")?
                .to_owned()),
        }
    }

    /// Number of displayed element in this tab.
    fn display_len(&self) -> usize {
        match self.display_mode {
            Display::Tree => self.tree.display_len(),
            Display::Preview => self.preview.len(),
            Display::Directory => self.directory.len(),
            Display::Flagged => 0,
        }
    }

    /// Path of the currently selected file.
    pub fn current_file_string(&self) -> Result<String> {
        Ok(path_to_string(&self.current_file()?.path))
    }

    /// Returns true if the current mode requires 2 windows.
    /// Only Tree, Normal & Preview doesn't require 2 windows.
    pub fn need_second_window(&self) -> bool {
        !matches!(self.edit_mode, Edit::Nothing)
    }

    /// Returns a string of the current directory path.
    pub fn directory_str(&self) -> String {
        path_to_string(&self.directory.path)
    }

    /// Returns a vector of filenames as strings, which contains the input string.
    /// Empty vector while in `Display::Preview`.
    pub fn filenames(&self, input_string: &str) -> Vec<String> {
        match self.display_mode {
            Display::Directory => self.directory.filenames_containing(input_string),
            Display::Tree => self.tree.filenames_containing(input_string),
            Display::Preview => vec![],
            Display::Flagged => vec![],
        }
    }

    /// Refresh everything but the view
    pub fn refresh_params(&mut self) -> Result<()> {
        self.preview = Preview::empty();
        if matches!(self.display_mode, Display::Tree) {
            self.make_tree(None)?;
        } else {
            self.tree = Tree::default()
        };
        Ok(())
    }

    /// Refresh the current view.
    /// displayed files is reset.
    /// The first file is selected.
    pub fn refresh_view(&mut self) -> Result<()> {
        self.directory.reset_files(&self.settings, &self.users)?;
        self.window.reset(self.display_len());
        self.refresh_params()?;
        Ok(())
    }

    /// Refresh the view if files were modified in current directory.
    /// If a refresh occurs, tries to select the same file as before.
    /// If it can't, the first file (`.`) is selected.
    /// Does nothing in `DisplayMode::Preview`.
    pub fn refresh_if_needed(&mut self) -> Result<()> {
        if match self.display_mode {
            Display::Preview => false,
            Display::Directory => {
                has_last_modification_happened_less_than(&self.directory.path, 10)?
            }
            Display::Tree => self.tree.has_modified_dirs(),
            Display::Flagged => false, // TODO! what to do ????,
        } {
            self.refresh_and_reselect_file()
        } else {
            Ok(())
        }
    }

    /// Change the display mode.
    pub fn set_display_mode(&mut self, new_display_mode: Display) {
        self.reset_preview();
        self.display_mode = new_display_mode
    }

    /// Makes a new tree of the current path.
    pub fn make_tree(&mut self, sort_kind: Option<SortKind>) -> Result<()> {
        let sort_kind = match sort_kind {
            Some(sort_kind) => sort_kind,
            None => SortKind::tree_default(),
        };
        self.settings.sort_kind = sort_kind.to_owned();
        let path = self.directory.path.clone();
        let users = &self.users;
        self.tree = Tree::new(
            path.clone(),
            5,
            sort_kind,
            users,
            self.settings.show_hidden,
            &self.settings.filter,
        );
        Ok(())
    }

    /// Enter or leave display tree mode.
    pub fn toggle_tree_mode(&mut self) -> Result<()> {
        let current_file = self.current_file()?;
        if let Display::Tree = self.display_mode {
            {
                self.tree = Tree::default();
                self.refresh_view()
            }?;
            self.set_display_mode(Display::Directory);
        } else {
            self.make_tree(None)?;
            self.window.reset(self.tree.displayable().lines().len());
            self.set_display_mode(Display::Tree);
        }
        self.go_to_file(current_file.path.to_path_buf());
        Ok(())
    }

    /// Creates a new preview for the selected file.
    pub fn make_preview(&mut self) -> Result<()> {
        if self.directory.is_empty() {
            return Ok(());
        }
        let Ok(file_info) = self.current_file() else {
            return Ok(());
        };
        match file_info.file_kind {
            FileKind::NormalFile => {
                let preview = Preview::file(&file_info).unwrap_or_default();
                self.set_display_mode(Display::Preview);
                self.window.reset(preview.len());
                self.preview = preview;
            }
            FileKind::Directory => self.toggle_tree_mode()?,
            _ => (),
        }

        Ok(())
    }

    /// Reset the preview to empty. Used to save some memory.
    fn reset_preview(&mut self) {
        if matches!(self.display_mode, Display::Preview) {
            self.preview = Preview::empty();
        }
    }

    /// Refresh the folder, reselect the last selected file, move the window to it.
    pub fn refresh_and_reselect_file(&mut self) -> Result<()> {
        let selected_path = self
            .current_file()
            .context("no selected file")?
            .path
            .clone();
        self.refresh_view()?;
        match self.display_mode {
            Display::Preview => (),
            Display::Directory => {
                let index = self.directory.select_file(&selected_path);
                self.scroll_to(index)
            }
            Display::Tree => {
                self.tree.go(To::Path(&selected_path));
                let index = self.tree.displayable().index();
                self.scroll_to(index);
            }
            Display::Flagged => {}
        }
        Ok(())
    }

    /// Reset the display mode and its view.
    pub fn reset_display_mode_and_view(&mut self) -> Result<()> {
        if matches!(self.display_mode, Display::Preview) {
            self.set_display_mode(Display::Directory);
        }
        self.refresh_view()
    }

    /// Set the height of the window and itself.
    pub fn set_height(&mut self, height: usize) {
        self.window.set_height(height);
        self.height = height;
    }

    /// Display or hide hidden files (filename starting with .).
    pub fn toggle_hidden(&mut self) -> Result<()> {
        self.settings.toggle_hidden();
        self.directory.reset_files(&self.settings, &self.users)?;
        self.window.reset(self.directory.content.len());
        if let Display::Tree = self.display_mode {
            self.make_tree(None)?
        }
        Ok(())
    }

    /// Set the window. Doesn't require the lenght to be known.
    pub fn set_window(&mut self) {
        let len = self.directory.content.len();
        self.window.reset(len);
    }

    /// Set the line index to `index` and scroll there.
    pub fn scroll_to(&mut self, index: usize) {
        self.window.scroll_to(index);
    }

    /// Sort the file with given criteria
    /// Valid kind of sorts are :
    /// by kind : directory first, files next, in alphanumeric order
    /// by filename,
    /// by date of modification,
    /// by size,
    /// by extension.
    /// The first letter is used to identify the method.
    /// If the user types an uppercase char, the sort is reverse.
    pub fn sort(&mut self, c: char) -> Result<()> {
        if self.directory.content.is_empty() {
            return Ok(());
        }
        // self.reset_edit_mode();
        match self.display_mode {
            Display::Directory => {
                let path = self.current_file()?.path;
                self.directory.unselect_current();
                self.settings.update_sort_from_char(c);
                crate::log_info!("sort kind: {sortkind}", sortkind = self.settings.sort_kind);
                self.directory.sort(&self.settings.sort_kind);
                self.normal_go_top();
                self.directory.select_file(&path);
            }
            Display::Tree => {
                self.settings.update_sort_from_char(c);
                let selected_path = self.tree.selected_path().to_owned();
                self.make_tree(Some(self.settings.sort_kind))?;
                self.tree.go(To::Path(&selected_path));
            }
            _ => (),
        }
        Ok(())
    }
    /// Set the pathcontent to a new path.
    /// Reset the window.
    /// Add the last path to the history of visited paths.
    /// Does nothing in preview or flagged display mode.
    pub fn cd(&mut self, path: &path::Path) -> Result<()> {
        if matches!(self.display_mode, Display::Preview | Display::Flagged) {
            return Ok(());
        }
        match std::env::set_current_dir(path) {
            Ok(()) => (),
            Err(error) => {
                crate::log_info!("can't reach {path}. Error {error}", path = path.display());
                return Ok(());
            }
        }
        self.history
            .push(&self.directory.path, &self.current_file()?.path);
        self.directory
            .change_directory(path, &self.settings, &self.users)?;
        if matches!(self.display_mode, Display::Tree) {
            self.make_tree(Some(self.settings.sort_kind))?;
            self.window.reset(self.tree.displayable().lines().len());
        } else {
            self.window.reset(self.directory.content.len());
        }
        Ok(())
    }

    pub fn back(&mut self) -> Result<()> {
        if matches!(self.display_mode, Display::Preview | Display::Flagged) {
            return Ok(());
        }
        if self.history.content.is_empty() {
            return Ok(());
        }
        let Some((path, file)) = self.history.content.pop() else {
            return Ok(());
        };
        self.history.content.pop();
        self.cd(&path)?;
        self.go_to_file(file);
        Ok(())
    }

    /// Select a file in current view, either directory or tree mode.
    pub fn go_to_file(&mut self, file: path::PathBuf) {
        if let Display::Tree = self.display_mode {
            self.tree.go(To::Path(&file));
        } else {
            let index = self.directory.select_file(&file);
            self.scroll_to(index);
        }
    }

    /// Jump to the jump target.
    /// Change the pathcontent and the tree if the jump target isn't in the
    /// currently displayed files.
    pub fn jump(&mut self, jump_target: path::PathBuf) -> Result<()> {
        let target_dir = match jump_target.parent() {
            Some(parent) => parent,
            None => &jump_target,
        };
        match self.display_mode {
            Display::Preview => return Ok(()),
            Display::Directory => {
                if !self.directory.paths().contains(&jump_target.as_path()) {
                    self.cd(target_dir)?
                }
                let index = self.directory.select_file(&jump_target);
                self.scroll_to(index)
            }
            Display::Tree => {
                if !self.tree.paths().contains(&target_dir) {
                    self.cd(target_dir)?;
                    self.make_tree(None)?
                }
                self.tree.go(To::Path(&jump_target))
            }
            Display::Flagged => {
                self.set_display_mode(Display::Directory);
                self.jump(jump_target)?;
            }
        }
        Ok(())
    }

    /// Move to the parent of current path
    pub fn move_to_parent(&mut self) -> Result<()> {
        let path = self.directory.path.clone();
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        if self.history.is_this_the_last(parent) {
            self.back()?;
            return Ok(());
        }
        self.cd(parent)
    }

    /// Select the file at index and move the window to this file.
    pub fn go_to_index(&mut self, index: usize) {
        self.directory.select_index(index);
        self.window.scroll_to(index);
    }

    /// Move to the currently selected directory.
    /// Fail silently if the current directory is empty or if the selected
    /// file isn't a directory.
    pub fn go_to_selected_dir(&mut self) -> Result<()> {
        self.cd(&self
            .directory
            .selected()
            .context("Empty directory")?
            .path
            .clone())?;
        Ok(())
    }

    /// Move down one row if possible.
    pub fn normal_down_one_row(&mut self) {
        self.directory.unselect_current();
        self.directory.next();
        self.directory.select_current();
        self.window.scroll_down_one(self.directory.index)
    }

    /// Move up one row if possible.
    pub fn normal_up_one_row(&mut self) {
        self.directory.unselect_current();
        self.directory.prev();
        self.directory.select_current();
        self.window.scroll_up_one(self.directory.index)
    }

    /// Move to the top of the current directory.
    pub fn normal_go_top(&mut self) {
        self.directory.select_index(0);
        self.window.scroll_to(0)
    }

    /// Move to the bottom of current view.
    pub fn normal_go_bottom(&mut self) {
        let last_index = self.directory.content.len() - 1;
        self.directory.select_index(last_index);
        self.window.scroll_to(last_index)
    }

    /// Move 10 files up
    pub fn normal_page_up(&mut self) {
        let up_index = if self.directory.index > 10 {
            self.directory.index - 10
        } else {
            0
        };
        self.directory.select_index(up_index);
        self.window.scroll_to(up_index)
    }

    /// Move down 10 rows
    pub fn normal_page_down(&mut self) {
        let down_index = min(self.directory.content.len() - 1, self.directory.index + 10);
        self.directory.select_index(down_index);
        self.window.scroll_to(down_index);
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node. Reset the display.
    pub fn tree_go_to_root(&mut self) -> Result<()> {
        self.tree.go(To::Root);
        self.window.scroll_to(0);
        Ok(())
    }

    /// Select the parent of current node.
    /// If we were at the root node, move to the parent and make a new tree.
    pub fn tree_select_parent(&mut self) -> Result<()> {
        if self.tree.is_on_root() {
            let Some(parent) = self.tree.root_path().parent() else {
                return Ok(());
            };
            self.cd(parent.to_owned().as_ref())?;
            self.make_tree(Some(self.settings.sort_kind))?;
        } else {
            self.tree.go(To::Parent);
        }
        let index = self.tree.displayable().index();
        self.window.scroll_to(index);
        Ok(())
    }

    /// Move down 10 times in the tree
    pub fn tree_page_down(&mut self) {
        self.tree.page_down();
        let index = self.tree.displayable().index();
        self.window.scroll_to(index);
    }

    /// Move up 10 times in the tree
    pub fn tree_page_up(&mut self) {
        self.tree.page_up();
        let index = self.tree.displayable().index();
        self.window.scroll_to(index);
    }

    /// Select the next sibling.
    pub fn tree_select_next(&mut self) -> Result<()> {
        self.tree.go(To::Next);
        let index = self.tree.displayable().index();
        self.window.scroll_down_one(index);
        Ok(())
    }

    /// Select the previous siblging
    pub fn tree_select_prev(&mut self) -> Result<()> {
        self.tree.go(To::Prev);
        let index = self.tree.displayable().index();
        self.window.scroll_up_one(index);
        Ok(())
    }

    /// Go to the last leaf.
    pub fn tree_go_to_bottom_leaf(&mut self) -> Result<()> {
        self.tree.go(To::Last);
        let index = self.tree.displayable().index();
        self.window.scroll_to(index);
        Ok(())
    }

    /// Navigate to the next sibling of current file in tree mode.
    pub fn tree_next_sibling(&mut self) {
        self.tree.go(To::NextSibling);
        let index = self.tree.displayable().index();
        self.window.scroll_to(index);
    }

    /// Navigate to the previous sibling of current file in tree mode.
    pub fn tree_prev_sibling(&mut self) {
        self.tree.go(To::PreviousSibling);
        let index = self.tree.displayable().index();
        self.window.scroll_to(index);
    }

    /// Move the preview to the top
    pub fn preview_go_top(&mut self) {
        self.window.scroll_to(0)
    }

    /// Move the preview to the bottom
    pub fn preview_go_bottom(&mut self) {
        self.window
            .scroll_to(self.preview.len().checked_sub(1).unwrap_or_default())
    }

    /// Move 30 lines up or an image in Ueberzug.
    pub fn preview_page_up(&mut self) {
        match &mut self.preview {
            Preview::Ueberzug(ref mut image) => image.up_one_row(),
            _ => {
                if self.window.top > 0 {
                    let skip = min(self.window.top, 30);
                    self.window.bottom -= skip;
                    self.window.top -= skip;
                }
            }
        }
    }

    /// Move down 30 rows except for Ueberzug where it moves 1 image down
    pub fn preview_page_down(&mut self) {
        match &mut self.preview {
            Preview::Ueberzug(ref mut image) => image.down_one_row(),
            _ => {
                if self.window.bottom < self.preview.len() {
                    let skip = min(self.preview.len() - self.window.bottom, 30);
                    self.window.bottom += skip;
                    self.window.top += skip;
                }
            }
        }
    }

    /// Select a given row, if there's something in it.
    /// Returns an error if the clicked row is above the headers margin.
    pub fn select_row(&mut self, row: u16) -> Result<()> {
        match self.display_mode {
            Display::Directory => self.normal_select_row(row),
            Display::Tree => self.tree_select_row(row)?,
            _ => (),
        }
        Ok(())
    }

    /// Select a clicked row in display directory
    fn normal_select_row(&mut self, row: u16) {
        let screen_index = row_to_window_index(row);
        let index = screen_index + self.window.top;
        self.directory.select_index(index);
        self.window.scroll_to(index);
    }

    /// Select a clicked row in display tree
    fn tree_select_row(&mut self, row: u16) -> Result<()> {
        let screen_index = row_to_window_index(row);
        let displayable = self.tree.displayable();
        let index = screen_index + self.window.top;
        let path = displayable
            .lines()
            .get(index)
            .context("no selected file")?
            .path()
            .to_owned();
        self.tree.go(To::Path(&path));
        Ok(())
    }

    /// Search in current directory for an file whose name contains `searched_name`,
    /// from a starting position `next_index`.
    /// We search forward from that position and start again from top if nothing is found.
    /// We move the selection to the first matching file.
    pub fn search_from(&mut self, searched_name: &str, current_index: usize) {
        if let Some(found_index) = self.search_from_index(searched_name, current_index) {
            self.go_to_index(found_index);
        } else if let Some(found_index) = self.search_from_top(searched_name, current_index) {
            self.go_to_index(found_index);
        }
    }

    /// Search a file by filename from given index, moving down
    fn search_from_index(&self, searched_name: &str, current_index: usize) -> Option<usize> {
        for (index, file) in self.directory.enumerate().skip(current_index) {
            if file.filename.contains(searched_name) {
                return Some(index);
            };
        }
        None
    }

    /// Search a file by filename from first line, moving down
    fn search_from_top(&self, searched_name: &str, current_index: usize) -> Option<usize> {
        for (index, file) in self.directory.enumerate().take(current_index) {
            if file.filename.contains(searched_name) {
                return Some(index);
            };
        }
        None
    }

    /// Search the next matching file in display directory
    pub fn normal_search_next(&mut self, searched: &str) {
        let next_index = (self.directory.index + 1) % self.directory.content.len();
        self.search_from(searched, next_index);
    }
}
