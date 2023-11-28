use std::cmp::min;
use std::path;

use anyhow::{Context, Result};

use crate::common::{has_last_modification_happened_less_than, row_to_window_index, set_clipboard};
use crate::config::Settings;
use crate::io::execute;
use crate::io::Args;
use crate::log_info;
use crate::modes::ContentWindow;
use crate::modes::FileInfo;
use crate::modes::FilterKind;
use crate::modes::History;
use crate::modes::PathContent;
use crate::modes::Preview;
use crate::modes::SelectableContent;
use crate::modes::Shortcut;
use crate::modes::SortKind;
use crate::modes::Users;
use crate::modes::{calculate_top_bottom, Go, To, Tree};
use crate::modes::{Display, Edit};

/// Holds every thing about the current tab of the application.
/// Most of the mutation is done externally.
pub struct Tab {
    /// Kind of display: `Preview, Normal, Tree`
    pub display_mode: Display,
    /// Files in current path
    pub path_content: PathContent,
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

    /// read from command line
    pub show_hidden: bool,
    /// The filter use before displaying files
    pub filter: FilterKind,
    /// The kind of sort used to display the files.
    pub sort_kind: SortKind,

    /// Predefined shortcuts
    pub shortcut: Shortcut,

    /// Last searched string
    pub searched: Option<String>,
    /// Visited directories
    pub history: History,
    /// Users & groups
    pub users: Users,

    /// True if the user issued a quit event (`Key::Char('q')` by default).
    /// It's used to exit the main loop before reseting the cursor.
    pub must_quit: bool,
}

impl Tab {
    /// Creates a new tab from args and height.
    pub fn new(
        args: &Args,
        height: usize,
        users: Users,
        settings: &Settings,
        mount_points: &[&path::Path],
    ) -> Result<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let start_dir = if path.is_dir() {
            &path
        } else {
            path.parent().context("")?
        };
        let filter = FilterKind::All;
        let show_hidden = args.all || settings.all;
        let mut path_content = PathContent::new(start_dir, &users, &filter, show_hidden)?;
        let display_mode = Display::default();
        let edit_mode = Edit::Nothing;
        let mut window = ContentWindow::new(path_content.content.len(), height);
        let must_quit = false;
        let preview = Preview::Empty;
        let history = History::default();
        let mut shortcut = Shortcut::new(&path);
        shortcut.extend_with_mount_points(mount_points);
        let searched = None;
        let index = path_content.select_file(&path);
        let tree = Tree::default();
        let sort_kind = SortKind::default();
        window.scroll_to(index);
        Ok(Self {
            display_mode,
            edit_mode,
            window,
            path_content,
            height,
            must_quit,
            preview,
            shortcut,
            searched,
            filter,
            show_hidden,
            history,
            users,
            tree,
            sort_kind,
        })
    }

    /// Refresh everything but the view
    pub fn refresh_params(&mut self) -> Result<()> {
        self.filter = FilterKind::All;
        self.preview = Preview::empty();
        self.set_edit_mode(Edit::Nothing);
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
        self.path_content
            .reset_files(&self.filter, self.show_hidden, &self.users)?;
        self.window.reset(self.path_content.content.len());
        self.refresh_params()?;
        Ok(())
    }

    /// Update the kind of sort from a char typed by the user.
    pub fn update_sort_from_char(&mut self, c: char) {
        self.sort_kind.update_from_char(c)
    }

    /// Refresh the view if files were modified in current directory.
    /// If a refresh occurs, tries to select the same file as before.
    /// If it can't, the first file (`.`) is selected.
    /// Does nothing in `DisplayMode::Preview`.
    pub fn refresh_if_needed(&mut self) -> Result<()> {
        if match self.display_mode {
            Display::Preview => false,
            Display::Normal => {
                has_last_modification_happened_less_than(&self.path_content.path, 10)?
            }
            Display::Tree => self.tree.has_modified_dirs(),
        } {
            self.refresh_and_reselect_file()
        } else {
            Ok(())
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
            Display::Normal => {
                let index = self.path_content.select_file(&selected_path);
                self.scroll_to(index)
            }
            Display::Tree => self.tree.go(To::Path(&selected_path)),
        }
        Ok(())
    }

    /// Move to the currently selected directory.
    /// Fail silently if the current directory is empty or if the selected
    /// file isn't a directory.
    pub fn go_to_selected_dir(&mut self) -> Result<()> {
        log_info!("go to selected");
        let childpath = &self
            .path_content
            .selected()
            .context("Empty directory")?
            .path
            .clone();
        log_info!("selected : {childpath:?}");
        self.cd(childpath)?;
        self.window.reset(self.path_content.content.len());
        Ok(())
    }

    /// Set the height of the window and itself.
    pub fn set_height(&mut self, height: usize) {
        self.window.set_height(height);
        self.height = height;
    }

    /// Returns `true` iff the application has to quit.
    /// This methods allows use to reset the cursors and other
    /// terminal parameters gracefully.
    pub fn must_quit(&self) -> bool {
        self.must_quit
    }

    /// Returns a string of the current directory path.
    pub fn path_content_str(&self) -> Option<&str> {
        self.path_content.path.to_str()
    }

    /// Set the pathcontent to a new path.
    /// Reset the window.
    /// Add the last path to the history of visited paths.
    pub fn cd(&mut self, path: &path::Path) -> Result<()> {
        self.history.push(
            &self.path_content.path,
            &self.path_content.selected().context("")?.path,
        );
        self.path_content.change_directory(
            path,
            &self.filter,
            self.show_hidden,
            &self.users,
            &self.sort_kind,
        )?;
        if matches!(self.display_mode, Display::Tree) {
            self.make_tree(Some(self.sort_kind))?;
        }
        self.window.reset(self.path_content.content.len());
        std::env::set_current_dir(path)?;
        Ok(())
    }

    /// Set the window. Doesn't require the lenght to be known.
    pub fn set_window(&mut self) {
        let len = self.path_content.content.len();
        self.window.reset(len);
    }
    /// Apply the filter.
    pub fn set_filter(&mut self, filter: FilterKind) {
        self.filter = filter
    }

    /// Set the line index to `index` and scroll there.
    pub fn scroll_to(&mut self, index: usize) {
        self.window.scroll_to(index);
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh_shortcuts(&mut self, mount_points: &[&path::Path]) {
        self.shortcut.refresh(mount_points)
    }

    /// Select the file at index and move the window to this file.
    pub fn go_to_index(&mut self, index: usize) {
        self.path_content.select_index(index);
        self.window.scroll_to(index);
    }

    /// Search in current directory for an file whose name contains `searched_name`,
    /// from a starting position `next_index`.
    /// We search forward from that position and start again from top if nothing is found.
    /// We move the selection to the first matching file.
    pub fn search_from(&mut self, searched_name: &str, current_index: usize) {
        let mut found = false;
        let mut next_index = current_index;
        // search after current position
        for (index, file) in self.path_content.enumerate().skip(current_index) {
            if file.filename.contains(searched_name) {
                next_index = index;
                found = true;
                break;
            };
        }
        if found {
            self.go_to_index(next_index);
            return;
        }

        // search from top
        for (index, file) in self.path_content.enumerate().take(current_index) {
            if file.filename.contains(searched_name) {
                next_index = index;
                found = true;
                break;
            };
        }
        if found {
            self.go_to_index(next_index)
        }
    }

    pub fn normal_search_next(&mut self, searched: &str) {
        let next_index = (self.path_content.index + 1) % self.path_content.content.len();
        self.search_from(searched, next_index);
    }

    /// Move to the parent of current path
    pub fn move_to_parent(&mut self) -> Result<()> {
        let path = self.path_content.path.clone();
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        if self.history.is_this_the_last(parent) {
            self.back()?;
            return Ok(());
        }
        self.cd(parent)
    }

    pub fn back(&mut self) -> Result<()> {
        if self.history.content.is_empty() {
            return Ok(());
        }
        let Some((path, file)) = self.history.content.pop() else {
            return Ok(());
        };
        self.cd(&path)?;
        let index = self.path_content.select_file(&file);
        self.scroll_to(index);
        self.history.content.pop();
        if let Display::Tree = self.display_mode {
            self.make_tree(None)?
        }

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
            self.make_tree(Some(self.sort_kind))
        } else {
            self.tree.go(To::Parent);
            Ok(())
        }
    }

    /// Move down 10 times in the tree
    pub fn tree_page_down(&mut self) -> Result<()> {
        self.tree.page_down();
        Ok(())
    }

    /// Move up 10 times in the tree
    pub fn tree_page_up(&mut self) {
        self.tree.page_up()
    }

    /// Select the next sibling.
    pub fn tree_select_next(&mut self) -> Result<()> {
        self.tree.go(To::Next);
        Ok(())
    }

    /// Select the previous siblging
    pub fn tree_select_prev(&mut self) -> Result<()> {
        self.tree.go(To::Prev);
        Ok(())
    }

    /// Go to the last leaf.
    pub fn tree_go_to_bottom_leaf(&mut self) -> Result<()> {
        self.tree.go(To::Last);
        Ok(())
    }

    /// Returns the directory owning the selected file.
    /// In Tree mode, it's the current directory if the selected node is a directory,
    /// its parent otherwise.
    /// In normal mode it's the current working directory.
    pub fn directory_of_selected(&self) -> Result<&path::Path> {
        match self.display_mode {
            Display::Tree => self.tree.directory_of_selected().context("No parent"),
            _ => Ok(&self.path_content.path),
        }
    }

    /// Fileinfo of the selected element.
    pub fn current_file(&self) -> Result<FileInfo> {
        match self.display_mode {
            Display::Tree => {
                let node = self.tree.selected_node().context("no selected node")?;
                node.fileinfo(&self.users)
            }
            _ => Ok(self
                .path_content
                .selected()
                .context("no selected file")?
                .to_owned()),
        }
    }

    /// Makes a new tree of the current path.
    pub fn make_tree(&mut self, sort_kind: Option<SortKind>) -> Result<()> {
        let sort_kind = match sort_kind {
            Some(sort_kind) => sort_kind,
            None => SortKind::tree_default(),
        };
        self.sort_kind = sort_kind.to_owned();
        let path = self.path_content.path.clone();
        let users = &self.users;
        self.tree = Tree::new(path, 5, sort_kind, users, self.show_hidden, &self.filter);
        Ok(())
    }

    /// Set a new mode and save the last one
    pub fn set_edit_mode(&mut self, new_mode: Edit) {
        self.edit_mode = new_mode;
    }

    pub fn set_display_mode(&mut self, new_display_mode: Display) {
        self.reset_preview();
        self.display_mode = new_display_mode
    }

    fn reset_preview(&mut self) {
        if matches!(self.display_mode, Display::Preview) {
            self.preview = Preview::empty();
        }
    }

    /// Reset the modes :
    /// - edit_mode is set to Nothing,
    pub fn reset_edit_mode(&mut self) -> bool {
        let must_refresh = matches!(self.display_mode, Display::Preview);
        self.edit_mode = Edit::Nothing;
        must_refresh
    }

    pub fn reset_mode_and_view(&mut self) -> Result<()> {
        if matches!(self.display_mode, Display::Preview) {
            self.set_display_mode(Display::Normal);
        }
        self.reset_edit_mode();
        self.refresh_view()
    }

    /// Returns true if the current mode requires 2 windows.
    /// Only Tree, Normal & Preview doesn't require 2 windows.
    pub fn need_second_window(&self) -> bool {
        !matches!(self.edit_mode, Edit::Nothing)
    }

    /// Move down one row if possible.
    pub fn normal_down_one_row(&mut self) {
        self.path_content.unselect_current();
        self.path_content.next();
        self.path_content.select_current();
        self.window.scroll_down_one(self.path_content.index)
    }

    /// Move up one row if possible.
    pub fn normal_up_one_row(&mut self) {
        self.path_content.unselect_current();
        self.path_content.prev();
        self.path_content.select_current();
        self.window.scroll_up_one(self.path_content.index)
    }

    /// Move to the top of the current directory.
    pub fn normal_go_top(&mut self) {
        self.path_content.select_index(0);
        self.window.scroll_to(0)
    }

    pub fn preview_go_top(&mut self) {
        self.window.scroll_to(0)
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node. Reset the display.
    pub fn tree_go_to_root(&mut self) -> Result<()> {
        self.tree.go(To::Root);
        Ok(())
    }

    /// Copy the selected filename to the clipboard. Only the filename.
    pub fn filename_to_clipboard(&self) {
        let Ok(file) = self.current_file() else {
            return;
        };
        set_clipboard(file.filename.clone())
    }

    /// Copy the selected filepath to the clipboard. The absolute path.
    pub fn filepath_to_clipboard(&self) {
        let Ok(file) = self.current_file() else {
            return;
        };
        let Some(path_str) = file.path.to_str() else {
            return;
        };
        set_clipboard(path_str.to_owned())
    }

    /// Move to the bottom of current view.
    pub fn normal_go_bottom(&mut self) {
        let last_index = self.path_content.content.len() - 1;
        self.path_content.select_index(last_index);
        self.window.scroll_to(last_index)
    }

    pub fn preview_go_bottom(&mut self) {
        self.window.scroll_to(self.preview.len() - 1)
    }

    /// Move 10 files up
    pub fn normal_page_up(&mut self) {
        let up_index = if self.path_content.index > 10 {
            self.path_content.index - 10
        } else {
            0
        };
        self.path_content.select_index(up_index);
        self.window.scroll_to(up_index)
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

    /// Move down 10 rows
    pub fn normal_page_down(&mut self) {
        let down_index = min(
            self.path_content.content.len() - 1,
            self.path_content.index + 10,
        );
        self.path_content.select_index(down_index);
        self.window.scroll_to(down_index);
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
    pub fn select_row(&mut self, row: u16, term_height: usize) -> Result<()> {
        match self.display_mode {
            Display::Normal => self.normal_select_row(row),
            Display::Tree => self.tree_select_row(row, term_height)?,
            _ => (),
        }
        Ok(())
    }

    fn normal_select_row(&mut self, row: u16) {
        let screen_index = row_to_window_index(row);
        let index = screen_index + self.window.top;
        self.path_content.select_index(index);
        self.window.scroll_to(index);
    }

    fn tree_select_row(&mut self, row: u16, term_height: usize) -> Result<()> {
        let screen_index = row_to_window_index(row);
        let (selected_index, content) = self.tree.into_navigable_content(&self.users);
        let (top, _) = calculate_top_bottom(selected_index, term_height - 2);
        let index = screen_index + top;
        let (_, _, colored_path) = content.get(index).context("no selected file")?;
        self.tree.go(To::Path(&colored_path.path));
        Ok(())
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
        if self.path_content.content.is_empty() {
            return Ok(());
        }
        self.reset_edit_mode();
        match self.display_mode {
            Display::Normal => {
                self.path_content.unselect_current();
                self.update_sort_from_char(c);
                self.path_content.sort(&self.sort_kind);
                self.normal_go_top();
                self.path_content.select_index(0);
            }
            Display::Tree => {
                self.update_sort_from_char(c);
                let selected_path = self.tree.selected_path().to_owned();
                self.make_tree(Some(self.sort_kind))?;
                self.tree.go(To::Path(&selected_path));
            }
            _ => (),
        }
        Ok(())
    }

    pub fn execute_custom(&self, exec_command: String) -> Result<bool> {
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        if !std::path::Path::new(command).exists() {
            return Ok(false);
        }
        let path = &self
            .path_content
            .selected_path_string()
            .context("execute custom: no selected file")?;
        args.push(path);
        execute(command, &args)?;
        Ok(true)
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
            Display::Normal => {
                if !self.path_content.paths().contains(&target_dir) {
                    self.cd(target_dir)?
                }
                let index = self.path_content.select_file(&jump_target);
                self.scroll_to(index)
            }
            Display::Tree => {
                if !self.tree.paths().contains(&target_dir) {
                    self.cd(target_dir)?;
                    self.make_tree(None)?
                }
                self.tree.go(To::Path(&jump_target))
            }
        }
        Ok(())
    }
}
