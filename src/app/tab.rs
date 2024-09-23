use std::borrow::Borrow;
use std::cmp::min;
use std::path;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::common::{
    has_last_modification_happened_less_than, path_to_string, row_to_window_index,
};
use crate::config::START_FOLDER;
use crate::io::{Args, Opener};
use crate::log_info;
use crate::modes::{
    Content, ContentWindow, Directory, Display, Edit, FileInfo, FileKind, FilterKind, Go, History,
    Preview, PreviewBuilder, Search, Selectable, SortKind, To, Tree, Users,
};

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

    pub fn reset_filter(&mut self) {
        self.filter = FilterKind::All;
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
    pub search: Search,
    // pub searched: Search,
    /// Visited directories
    pub history: History,
    /// Users & groups
    pub users: Users,
}

impl Tab {
    /// Creates a new tab from args and height.
    ///
    /// # Description
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
        let path = &START_FOLDER.get().context("Startfolder should be set")?;
        let start_dir = Self::start_dir(path)?;
        let settings = TabSettings::new(args);
        let mut directory =
            Directory::new(start_dir, &users, &settings.filter, settings.show_hidden)?;
        let display_mode = Display::default();
        let edit_mode = Edit::Nothing;
        let mut window = ContentWindow::new(directory.content.len(), height);
        let preview = Preview::Empty;
        let history = History::default();
        let search = Search::empty();
        let index = directory.select_file(path);
        let tree = Tree::default();

        window.scroll_to(index);
        Ok(Self {
            display_mode,
            edit_mode,
            window,
            directory,
            height,
            preview,
            search,
            history,
            users,
            tree,
            settings,
        })
    }

    fn start_dir(path: &path::Path) -> Result<&path::Path> {
        if path.is_dir() {
            Ok(path)
        } else {
            Ok(path.parent().context("Path has no parent")?)
        }
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

    /// Current path of this tab in directory display mode.
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
                .context("current_file: no selected file")?
                .to_owned()),
        }
    }

    /// Number of displayed element in this tab.
    fn display_len(&self) -> usize {
        match self.display_mode {
            Display::Tree => self.tree.display_len(),
            Display::Preview => self.preview.len(),
            Display::Directory => self.directory.len(),
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

    /// Refresh everything but the view
    pub fn refresh_params(&mut self) {
        self.preview = PreviewBuilder::empty();
        if matches!(self.display_mode, Display::Tree) {
            self.make_tree(None);
        } else {
            self.tree = Tree::default()
        };
    }

    /// Refresh the current view.
    /// displayed files is reset.
    /// The first file is selected.
    pub fn refresh_view(&mut self) -> Result<()> {
        self.directory.reset_files(&self.settings, &self.users)?;
        self.window.reset(self.display_len());
        self.refresh_params();
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
        } {
            self.refresh_and_reselect_file()
        } else {
            Ok(())
        }
    }

    /// Change the display mode.
    pub fn set_display_mode(&mut self, new_display_mode: Display) {
        self.search.reset_paths();
        self.reset_preview();
        self.display_mode = new_display_mode
    }

    /// Makes a new tree of the current path.
    pub fn make_tree(&mut self, sort_kind: Option<SortKind>) {
        let sort_kind = sort_kind.unwrap_or_default();
        self.settings.sort_kind = sort_kind;
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
    }

    fn make_tree_for_parent(&mut self) -> Result<()> {
        let Some(parent) = self.tree.root_path().parent() else {
            return Ok(());
        };
        self.cd(parent.to_owned().as_ref())?;
        self.make_tree(Some(self.settings.sort_kind));
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
            self.make_tree(None);
            self.window.reset(self.tree.displayable().lines().len());
            self.set_display_mode(Display::Tree);
        }
        self.go_to_file(current_file.path);
        Ok(())
    }

    /// Creates a new preview for the selected file.
    /// If the selected file is a directory, it will create a tree.
    /// Does nothing if directory is empty or in flagged or preview display mode.
    pub fn make_preview(&mut self) -> Result<()> {
        if self.directory.is_empty() {
            return Ok(());
        }
        let Ok(file_info) = self.current_file() else {
            return Ok(());
        };
        match file_info.file_kind {
            FileKind::Directory => self.toggle_tree_mode()?,
            _ => self.make_preview_unchecked(file_info),
        }

        Ok(())
    }

    /// Creates a preview and assign it.
    /// Doesn't check if it's the correct action to do according to display.
    fn make_preview_unchecked(&mut self, file_info: FileInfo) {
        let preview = PreviewBuilder::new(&file_info.path, &self.users)
            .build()
            .unwrap_or_default();
        self.set_display_mode(Display::Preview);
        self.window.reset(preview.len());
        self.preview = preview;
    }

    /// Reset the preview to empty. Used to save some memory.
    fn reset_preview(&mut self) {
        if matches!(self.display_mode, Display::Preview) {
            self.preview = PreviewBuilder::empty();
        }
    }

    /// Refresh the folder, reselect the last selected file, move the window to it.
    pub fn refresh_and_reselect_file(&mut self) -> Result<()> {
        let selected_path = self.clone_selected_path()?;
        self.refresh_view()?;
        self.reselect(selected_path);
        Ok(())
    }

    fn clone_selected_path(&self) -> Result<Arc<path::Path>> {
        Ok(self
            .current_file()
            .context("refresh: no selected file")?
            .path
            .clone())
    }

    fn reselect(&mut self, selected_path: Arc<path::Path>) {
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
        }
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
            self.make_tree(None)
        }
        Ok(())
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
        match self.display_mode {
            Display::Directory => self.sort_directory(c)?,
            Display::Tree => self.sort_tree(c),
            _ => (),
        }
        Ok(())
    }

    fn sort_directory(&mut self, c: char) -> Result<()> {
        let path = self.current_file()?.path;
        self.settings.update_sort_from_char(c);
        self.directory.sort(&self.settings.sort_kind);
        self.normal_go_top();
        self.directory.select_file(&path);
        Ok(())
    }

    fn sort_tree(&mut self, c: char) {
        self.settings.update_sort_from_char(c);
        let selected_path = self.tree.selected_path().to_owned();
        self.make_tree(Some(self.settings.sort_kind));
        self.tree.go(To::Path(&selected_path));
    }

    pub fn cd_to_file(&mut self, path: &path::Path) -> Result<()> {
        let parent = path.parent().context("no parent")?;
        self.cd(parent)?;
        self.go_to_file(path);
        Ok(())
    }

    /// Set the pathcontent to a new path.
    /// Reset the window.
    /// Add the last path to the history of visited paths.
    /// Does nothing in preview or flagged display mode.
    pub fn cd(&mut self, path: &path::Path) -> Result<()> {
        if matches!(self.display_mode, Display::Preview) {
            return Ok(());
        }
        self.search.reset_paths();
        match std::env::set_current_dir(path) {
            Ok(()) => (),
            Err(error) => {
                log_info!("can't reach {path}. Error {error}", path = path.display());
                return Ok(());
            }
        }
        self.history
            .push(&self.directory.path, &self.current_file()?.path);
        self.directory
            .change_directory(path, &self.settings, &self.users)?;
        if matches!(self.display_mode, Display::Tree) {
            self.make_tree(Some(self.settings.sort_kind));
            self.window.reset(self.tree.displayable().lines().len());
        } else {
            self.window.reset(self.directory.content.len());
        }
        Ok(())
    }

    pub fn back(&mut self) -> Result<()> {
        if matches!(self.display_mode, Display::Preview) {
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
    pub fn go_to_file<P>(&mut self, file: P)
    where
        P: AsRef<path::Path>,
    {
        if let Display::Tree = self.display_mode {
            self.tree.go(To::Path(file.as_ref()));
        } else {
            let index = self.directory.select_file(file.as_ref());
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
            Display::Directory => self.jump_directory(&jump_target, target_dir)?,
            Display::Tree => self.jump_tree(&jump_target, target_dir)?,
        }
        Ok(())
    }

    fn jump_directory(&mut self, jump_target: &path::Path, target_dir: &path::Path) -> Result<()> {
        if !self.directory.paths().contains(&jump_target) {
            self.cd(target_dir)?
        }
        let index = self.directory.select_file(jump_target);
        self.scroll_to(index);
        Ok(())
    }

    fn jump_tree(&mut self, jump_target: &path::Path, target_dir: &path::Path) -> Result<()> {
        if !self.tree.paths().contains(&target_dir) {
            self.cd(target_dir)?;
            self.make_tree(None);
        }
        self.tree.go(To::Path(jump_target));
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
        self.cd_to_file(&path)
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
        self.directory.next();
        self.window.scroll_down_one(self.directory.index)
    }

    /// Move up one row if possible.
    pub fn normal_up_one_row(&mut self) {
        self.directory.prev();
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
        let up_index = self.directory.index.saturating_sub(10);
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
            self.make_tree_for_parent()?;
        } else {
            self.tree.go(To::Parent);
        }
        self.window.scroll_to(self.tree.displayable().index());
        Ok(())
    }

    /// Move down 10 times in the tree
    pub fn tree_page_down(&mut self) {
        self.tree.page_down();
        self.window.scroll_to(self.tree.displayable().index());
    }

    /// Move up 10 times in the tree
    pub fn tree_page_up(&mut self) {
        self.tree.page_up();
        self.window.scroll_to(self.tree.displayable().index());
    }

    /// Select the next sibling.
    pub fn tree_select_next(&mut self) {
        self.tree.go(To::Next);
        self.window.scroll_down_one(self.tree.displayable().index());
    }

    /// Select the previous siblging
    pub fn tree_select_prev(&mut self) {
        self.tree.go(To::Prev);
        self.window.scroll_up_one(self.tree.displayable().index());
    }

    /// Go to the last leaf.
    pub fn tree_go_to_bottom_leaf(&mut self) {
        self.tree.go(To::Last);
        self.window.scroll_to(self.tree.displayable().index());
    }

    /// Navigate to the next sibling of current file in tree mode.
    pub fn tree_next_sibling(&mut self) {
        self.tree.go(To::NextSibling);
        self.window.scroll_to(self.tree.displayable().index());
    }

    /// Navigate to the previous sibling of current file in tree mode.
    pub fn tree_prev_sibling(&mut self) {
        self.tree.go(To::PreviousSibling);
        self.window.scroll_to(self.tree.displayable().index());
    }

    /// Move the preview to the top
    pub fn preview_go_top(&mut self) {
        self.window.scroll_to(0)
    }

    /// Move the preview to the bottom
    pub fn preview_go_bottom(&mut self) {
        self.window.scroll_to(self.preview.len().saturating_sub(1))
    }

    /// Move 30 lines up or an image in Ueberzug.
    pub fn preview_page_up(&mut self) {
        match &mut self.preview {
            Preview::Ueberzug(ref mut image) => image.up_one_row(),
            _ => self.window.preview_page_up(),
        }
    }

    /// Move down 30 rows except for Ueberzug where it moves 1 image down
    pub fn preview_page_down(&mut self) {
        match &mut self.preview {
            Preview::Ueberzug(ref mut image) => image.down_one_row(),
            _ => self.window.preview_page_down(self.preview.len()),
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
            .context("tree: no selected file")?
            .path()
            .to_owned();
        self.tree.go(To::Path(&path));
        Ok(())
    }

    pub fn directory_search_next(&mut self) {
        if let Some(path) = self.search.select_next() {
            self.go_to_file(path)
        } else if let (paths, Some(index), Some(path)) = self.search.directory_search_next(self) {
            self.search.set_index_paths(index, paths);
            self.go_to_file(path);
        }
    }

    pub fn context_info(&self, opener: &Opener) -> Vec<String> {
        let Ok(selected) = self.current_file() else {
            return vec![];
        };
        selected.context_info(opener)
    }
}
