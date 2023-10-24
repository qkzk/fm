use std::cmp::min;
use std::path;

use anyhow::{Context, Result};
use users::UsersCache;

use crate::args::Args;
use crate::completion::{Completion, InputCompleted};
use crate::config::{Colors, Settings};
use crate::content_window::ContentWindow;
use crate::fileinfo::{FileInfo, PathContent};
use crate::filter::FilterKind;
use crate::history::History;
use crate::input::Input;
use crate::mode::{InputSimple, Mode};
use crate::opener::execute_in_child;
use crate::preview::{Directory, Preview};
use crate::selectable_content::SelectableContent;
use crate::shortcut::Shortcut;
use crate::utils::{filename_from_path, row_to_window_index, set_clipboard};

/// Holds every thing about the current tab of the application.
/// Most of the mutation is done externally.
pub struct Tab {
    /// The mode the application is currenty in
    pub mode: Mode,
    /// The mode previously set
    pub previous_mode: Mode,
    /// The indexes of displayed file
    pub window: ContentWindow,
    /// The typed input by the user
    pub input: Input,
    /// Files in current path
    pub path_content: PathContent,
    /// Height of the terminal window
    pub height: usize,
    /// read from command line
    pub show_hidden: bool,
    /// Completion list and index in it.
    pub completion: Completion,
    /// True if the user issued a quit event (`Key::Char('q')` by default).
    /// It's used to exit the main loop before reseting the cursor.
    pub must_quit: bool,
    /// Lines of the previewed files.
    /// Empty if not in preview mode.
    pub preview: Preview,
    /// Predefined shortcuts
    pub shortcut: Shortcut,
    /// Last searched string
    pub searched: Option<String>,
    /// Optional tree view
    pub directory: Directory,
    /// The filter use before displaying files
    pub filter: FilterKind,
    /// Visited directories
    pub history: History,
}

impl Tab {
    /// Creates a new tab from args and height.
    pub fn new(
        args: &Args,
        height: usize,
        users_cache: UsersCache,
        settings: &Settings,
        mount_points: &[&path::Path],
    ) -> Result<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let start_dir = if path.is_dir() {
            &path
        } else {
            path.parent().context("")?
        };
        let directory = Directory::empty(start_dir, &users_cache)?;
        let filter = FilterKind::All;
        let show_hidden = args.all || settings.all;
        let mut path_content = PathContent::new(start_dir, users_cache, &filter, show_hidden)?;
        let mode = Mode::Normal;
        let previous_mode = Mode::Normal;
        let mut window = ContentWindow::new(path_content.content.len(), height);
        let input = Input::default();
        let completion = Completion::default();
        let must_quit = false;
        let preview = Preview::Empty;
        let history = History::default();
        let mut shortcut = Shortcut::new(&path);
        shortcut.extend_with_mount_points(mount_points);
        let searched = None;
        let index = path_content.select_file(&path);
        window.scroll_to(index);
        Ok(Self {
            mode,
            previous_mode,
            window,
            input,
            path_content,
            height,
            completion,
            must_quit,
            preview,
            shortcut,
            searched,
            directory,
            filter,
            show_hidden,
            history,
        })
    }

    /// Fill the input string with the currently selected completion.
    pub fn fill_completion(&mut self) -> Result<()> {
        match self.mode {
            Mode::InputCompleted(InputCompleted::Goto) => {
                let current_path = self.path_content_str().unwrap_or_default().to_owned();
                self.completion.goto(&self.input.string(), &current_path)
            }
            Mode::InputCompleted(InputCompleted::Exec) => {
                self.completion.exec(&self.input.string())
            }
            Mode::InputCompleted(InputCompleted::Search)
                if matches!(self.previous_mode, Mode::Normal) =>
            {
                self.completion
                    .search_from_normal(&self.input.string(), &self.path_content)
            }
            Mode::InputCompleted(InputCompleted::Search)
                if matches!(self.previous_mode, Mode::Tree) =>
            {
                self.completion
                    .search_from_tree(&self.input.string(), &self.directory.content)
            }
            Mode::InputCompleted(InputCompleted::Command) => {
                self.completion.command(&self.input.string())
            }
            _ => Ok(()),
        }
    }

    /// Refresh everything but the view
    pub fn refresh_params(&mut self) -> Result<()> {
        self.filter = FilterKind::All;
        self.input.reset();
        self.preview = Preview::new_empty();
        self.completion.reset();
        self.directory.clear();
        Ok(())
    }

    /// Refresh the current view.
    /// Input string is emptied, the files are read again, the window of
    /// displayed files is reset.
    /// The first file is selected.
    pub fn refresh_view(&mut self) -> Result<()> {
        self.refresh_params()?;
        self.path_content
            .reset_files(&self.filter, self.show_hidden)?;
        self.window.reset(self.path_content.content.len());
        Ok(())
    }

    /// Refresh the view if files were modified in current directory.
    /// If a refresh occurs, tries to select the same file as before.
    /// If it can't, the first file (`.`) is selected.
    /// Does nothing outside of normal mode.
    pub fn refresh_if_needed(&mut self) -> Result<()> {
        if let Mode::Normal = self.mode {
            if self.is_last_modification_happend_less_than(10)? {
                self.refresh_and_reselect_file()?
            }
        }
        Ok(())
    }

    /// True iff the last modification of current folder happened less than `seconds` ago.
    fn is_last_modification_happend_less_than(&self, seconds: u64) -> Result<bool> {
        Ok(self.path_content.path.metadata()?.modified()?.elapsed()?
            < std::time::Duration::new(seconds, 0))
    }

    /// Refresh the folder, reselect the last selected file, move the window to it.
    fn refresh_and_reselect_file(&mut self) -> Result<()> {
        let selected_path = self.selected().context("no selected file")?.path.clone();
        self.refresh_view()?;
        let index = self.path_content.select_file(&selected_path);
        self.scroll_to(index);
        Ok(())
    }

    /// Move to the currently selected directory.
    /// Fail silently if the current directory is empty or if the selected
    /// file isn't a directory.
    pub fn go_to_child(&mut self) -> Result<()> {
        let childpath = &self
            .path_content
            .selected()
            .context("Empty directory")?
            .path
            .clone();
        self.set_pathcontent(childpath)?;
        self.window.reset(self.path_content.content.len());
        self.input.cursor_start();
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
    pub fn set_pathcontent(&mut self, path: &path::Path) -> Result<()> {
        self.history.push(
            &self.path_content.path,
            &self.path_content.selected().context("")?.path,
        );
        self.path_content
            .change_directory(path, &self.filter, self.show_hidden)?;
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

    /// Refresh the existing users.
    pub fn refresh_users(&mut self, users_cache: UsersCache) -> Result<()> {
        let last_pathcontent_index = self.path_content.index;
        self.path_content
            .refresh_users(users_cache, &self.filter, self.show_hidden)?;
        self.path_content.select_index(last_pathcontent_index);
        Ok(())
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

    /// Select the root node of the tree.
    pub fn tree_select_root(&mut self, colors: &Colors) -> Result<()> {
        self.directory.unselect_children();
        self.directory.select_root(colors)
    }

    /// Move to the parent of current path
    pub fn move_to_parent(&mut self, colors: &Colors) -> Result<()> {
        let path = self.path_content.path.clone();
        let Some(parent) = path.parent() else {
            return Ok(());
        };
        if self.history.is_this_the_last(parent) {
            self.back(colors)?;
            return Ok(());
        }
        self.set_pathcontent(parent)
    }

    pub fn back(&mut self, colors: &Colors) -> Result<()> {
        let Some((path, file)) = self.history.content.pop() else {
            return Ok(());
        };
        self.set_pathcontent(&path)?;
        let index = self.path_content.select_file(&file);
        self.scroll_to(index);
        self.history.content.pop();
        if let Mode::Tree = self.mode {
            self.make_tree(colors)?
        }

        Ok(())
    }

    /// Select the parent of current node.
    /// If we were at the root node, move to the parent and make a new tree.
    pub fn tree_select_parent(&mut self, colors: &Colors) -> Result<()> {
        self.directory.unselect_children();
        if self.directory.tree.position.len() <= 1 {
            self.move_to_parent(colors)?;
            self.make_tree(colors)?
        }
        self.directory.select_parent(colors)
    }

    /// Move down 10 times in the tree
    pub fn tree_page_down(&mut self, colors: &Colors) -> Result<()> {
        self.directory.tree.increase_required_height_by_ten();
        self.directory.unselect_children();
        self.directory.page_down(colors)
    }

    /// Move up 10 times in the tree
    pub fn tree_page_up(&mut self, colors: &Colors) -> Result<()> {
        self.directory.tree.decrease_required_height_by_ten();
        self.directory.unselect_children();
        self.directory.page_up(colors)
    }

    /// Select the next sibling.
    pub fn tree_select_next(&mut self, colors: &Colors) -> Result<()> {
        self.directory.select_next(colors)
    }

    /// Select the previous siblging
    pub fn tree_select_prev(&mut self, colors: &Colors) -> Result<()> {
        self.directory.select_prev(colors)
    }

    /// Select the first child if any.
    pub fn tree_select_first_child(&mut self, colors: &Colors) -> Result<()> {
        self.directory.unselect_children();
        self.directory.select_first_child(colors)
    }

    /// Go to the last leaf.
    pub fn tree_go_to_bottom_leaf(&mut self, colors: &Colors) -> Result<()> {
        self.directory.tree.set_required_height_to_max();
        self.directory.unselect_children();
        self.directory.go_to_bottom_leaf(colors)
    }

    /// Returns the current path.
    /// In tree mode :
    ///     if the selected node is a directory, that's it.
    ///     else, it is the parent of the selected node.
    /// In other modes, it's the current path of pathcontent.
    pub fn current_directory_path(&mut self) -> &path::Path {
        match self.mode {
            Mode::Tree => {
                let path = &self.directory.tree.current_node.fileinfo.path;
                if path.is_dir() {
                    return path;
                }
                let Some(parent) = path.parent() else {
                    return path::Path::new("/");
                };
                parent
            }
            _ => &self.path_content.path,
        }
    }

    /// Returns the directory owning the selected file.
    /// In Tree mode, it's the current directory if the selected node is a directory,
    /// its parent otherwise.
    /// In normal mode it's the current working directory.
    pub fn directory_of_selected(&self) -> Result<&path::Path> {
        match self.mode {
            Mode::Tree => self.directory.tree.directory_of_selected(),
            _ => Ok(&self.path_content.path),
        }
    }

    /// Optional Fileinfo of the selected element.
    pub fn selected(&self) -> Option<&FileInfo> {
        match self.mode {
            Mode::Tree => Some(&self.directory.tree.current_node.fileinfo),
            _ => self.path_content.selected(),
        }
    }

    /// Makes a new tree of the current path.
    pub fn make_tree(&mut self, colors: &Colors) -> Result<()> {
        let path = self.path_content.path.clone();
        let users_cache = &self.path_content.users_cache;
        self.directory = Directory::new(
            &path,
            users_cache,
            colors,
            &self.filter,
            self.show_hidden,
            None,
        )?;
        Ok(())
    }

    /// Set a new mode and save the last one
    pub fn set_mode(&mut self, new_mode: Mode) {
        self.previous_mode = self.mode;
        self.mode = new_mode;
    }

    /// Reset the last mode.
    /// The last mode is set to normal again.
    /// Returns True if the last mode requires a refresh afterwards.
    pub fn reset_mode(&mut self) -> bool {
        let must_refresh = self.mode.refresh_required();
        self.mode = self.previous_mode;
        self.previous_mode = Mode::Normal;
        must_refresh
    }

    /// Returns true if the current mode requires 2 windows.
    /// Only Tree, Normal & Preview doesn't require 2 windows.
    pub fn need_second_window(&self) -> bool {
        !matches!(self.mode, Mode::Normal | Mode::Tree | Mode::Preview)
    }

    /// Move down one row if possible.
    pub fn down_one_row(&mut self) {
        match self.mode {
            Mode::Normal => {
                self.path_content.unselect_current();
                self.path_content.next();
                self.path_content.select_current();
                self.window.scroll_down_one(self.path_content.index)
            }
            Mode::Preview => self.preview_page_down(),
            _ => (),
        }
    }

    /// Move up one row if possible.
    pub fn up_one_row(&mut self) {
        match self.mode {
            Mode::Normal => {
                self.path_content.unselect_current();
                self.path_content.prev();
                self.path_content.select_current();
                self.window.scroll_up_one(self.path_content.index)
            }
            Mode::Preview => self.preview_page_up(),
            _ => (),
        }
    }

    /// Move to the top of the current directory.
    pub fn go_top(&mut self) {
        match self.mode {
            Mode::Normal => self.path_content.select_index(0),
            Mode::Preview => (),
            _ => {
                return;
            }
        }
        self.window.scroll_to(0);
    }

    /// Add a char to input string, look for a possible completion.
    pub fn text_insert_and_complete(&mut self, c: char) -> Result<()> {
        self.input.insert(c);
        self.fill_completion()
    }

    /// Fold every child node in the tree.
    /// Recursively explore the tree and fold every node. Reset the display.
    pub fn tree_go_to_root(&mut self, colors: &Colors) -> Result<()> {
        self.directory.tree.reset_required_height();
        self.tree_select_root(colors)
    }

    /// Select the first child of the current node and reset the display.
    pub fn select_first_child(&mut self, colors: &Colors) -> Result<()> {
        self.tree_select_first_child(colors)
    }

    /// Select the next sibling of the current node.
    pub fn select_next(&mut self, colors: &Colors) -> Result<()> {
        self.tree_select_next(colors)
    }

    /// Select the previous sibling of the current node.
    pub fn select_prev(&mut self, colors: &Colors) -> Result<()> {
        self.tree_select_prev(colors)
    }

    /// Copy the selected filename to the clipboard. Only the filename.
    pub fn filename_to_clipboard(&self) -> Result<()> {
        set_clipboard(
            self.selected()
                .context("filename_to_clipboard: no selected file")?
                .filename
                .clone(),
        )
    }

    /// Copy the selected filepath to the clipboard. The absolute path.
    pub fn filepath_to_clipboard(&self) -> Result<()> {
        set_clipboard(
            self.selected()
                .context("filepath_to_clipboard: no selected file")?
                .path
                .to_str()
                .context("filepath_to_clipboard: no selected file")?
                .to_owned(),
        )
    }

    /// Move to the bottom of current view.
    pub fn go_bottom(&mut self) {
        match self.mode {
            Mode::Normal => {
                let last_index = self.path_content.content.len() - 1;
                self.path_content.select_index(last_index);
                self.window.scroll_to(last_index)
            }
            Mode::Preview => self.window.scroll_to(self.preview.len() - 1),
            _ => (),
        }
    }

    /// Move up 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves up one page.
    pub fn page_up(&mut self) {
        match self.mode {
            Mode::Normal => {
                let up_index = if self.path_content.index > 10 {
                    self.path_content.index - 10
                } else {
                    0
                };
                self.path_content.select_index(up_index);
                self.window.scroll_to(up_index)
            }
            Mode::Preview => self.preview_page_up(),
            _ => (),
        }
    }

    fn preview_page_up(&mut self) {
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

    /// Move down 10 rows in normal mode.
    /// In other modes where vertical scrolling is possible (atm Preview),
    /// if moves down one page.
    pub fn page_down(&mut self) {
        match self.mode {
            Mode::Normal => self.normal_page_down(),
            Mode::Preview => self.preview_page_down(),
            _ => (),
        }
    }

    fn normal_page_down(&mut self) {
        let down_index = min(
            self.path_content.content.len() - 1,
            self.path_content.index + 10,
        );
        self.path_content.select_index(down_index);
        self.window.scroll_to(down_index);
    }

    fn preview_page_down(&mut self) {
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
    pub fn select_row(&mut self, row: u16, colors: &Colors, term_height: usize) -> Result<()> {
        match self.mode {
            Mode::Normal => self.normal_select_row(row),
            Mode::Tree => self.tree_select_row(row, colors, term_height)?,
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

    fn tree_select_row(&mut self, row: u16, colors: &Colors, term_height: usize) -> Result<()> {
        let screen_index = row_to_window_index(row) + 1;
        // term.height = canvas.height + 2 rows for the canvas border
        let (top, _, _) = self.directory.calculate_tree_window(term_height - 2);
        let index = screen_index + top;
        self.directory.tree.unselect_children();
        self.directory.tree.position = self.directory.tree.position_from_index(index);
        let (_, _, node) = self.directory.tree.select_from_position()?;
        self.directory.make_preview(colors);
        self.directory.tree.current_node = node;
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
    pub fn sort(&mut self, c: char, colors: &Colors) -> Result<()> {
        if self.path_content.content.is_empty() {
            return Ok(());
        }
        self.reset_mode();
        match self.mode {
            Mode::Normal => {
                self.path_content.unselect_current();
                self.path_content.update_sort_from_char(c);
                self.path_content.sort();
                self.go_top();
                self.path_content.select_index(0);
            }
            Mode::Tree => {
                self.directory.tree.update_sort_from_char(c);
                self.directory.tree.sort();
                self.tree_select_root(colors)?;
                self.directory.tree.into_navigable_content(colors);
            }
            _ => (),
        }
        Ok(())
    }

    pub fn execute_custom(&mut self, exec_command: String) -> Result<bool> {
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        if !std::path::Path::new(command).exists() {
            return Ok(false);
        }
        let path = &self
            .path_content
            .selected_path_string()
            .context("execute custom: can't find command")?;
        args.push(path);
        execute_in_child(command, &args)?;
        Ok(true)
    }

    pub fn rename(&mut self) -> Result<()> {
        if self.selected().is_some() {
            let old_name = match self.mode {
                Mode::Tree => self.directory.tree.current_node.filename(),
                _ => filename_from_path(
                    &self
                        .path_content
                        .selected()
                        .context("Event rename: no file in current directory")?
                        .path,
                )?
                .to_owned(),
            };
            self.input.replace(&old_name);
            self.set_mode(Mode::InputSimple(InputSimple::Rename));
        }
        Ok(())
    }
}
