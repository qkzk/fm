use std::path;
use std::rc::Rc;

use users::UsersCache;

use crate::args::Args;
use crate::completion::Completion;
use crate::config::Colors;
use crate::content_window::ContentWindow;
use crate::fileinfo::{FileInfo, FileKind, PathContent};
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::input::Input;
use crate::mode::Mode;
use crate::preview::{Directory, Preview};
use crate::selectable_content::SelectableContent;
use crate::shortcut::Shortcut;
use crate::visited::History;

/// Holds every thing about the current tab of the application.
/// Most of the mutation is done externally.
#[derive(Clone)]
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
    /// NVIM RPC server address
    pub nvim_server: String,
    /// Completion list and index in it.
    pub completion: Completion,
    /// True if the user issued a quit event (`Key::Char('q')` by default).
    /// It's used to exit the main loop before reseting the cursor.
    pub must_quit: bool,
    /// Lines of the previewed files.
    /// Empty if not in preview mode.
    pub preview: Preview,
    /// Visited directories
    pub history: History,
    /// Predefined shortcuts
    pub shortcut: Shortcut,
    /// Last searched string
    pub searched: Option<String>,
    /// Optional tree view
    pub directory: Directory,
    /// The filter use before displaying files
    pub filter: FilterKind,
}

impl Tab {
    /// Creates a new tab from args and height.
    pub fn new(args: Args, height: usize, users_cache: Rc<UsersCache>) -> FmResult<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let directory = Directory::empty(&path, &users_cache)?;
        let filter = FilterKind::All;
        let show_hidden = false;
        let path_content = PathContent::new(&path, users_cache, &filter, show_hidden)?;
        let show_hidden = false;
        let nvim_server = args.server;
        let mode = Mode::Normal;
        let previous_mode = Mode::Normal;
        let window = ContentWindow::new(path_content.content.len(), height);
        let input = Input::default();
        let completion = Completion::default();
        let must_quit = false;
        let preview = Preview::Empty;
        let mut history = History::default();
        history.push(&path);
        let shortcut = Shortcut::new();
        let searched = None;
        Ok(Self {
            mode,
            previous_mode,
            window,
            input,
            path_content,
            height,
            nvim_server,
            completion,
            must_quit,
            preview,
            history,
            shortcut,
            searched,
            directory,
            filter,
            show_hidden,
        })
    }

    /// Fill the input string with the currently selected completion.
    pub fn fill_completion(&mut self) -> FmResult<()> {
        self.completion.set_kind(&self.mode);
        let current_path = self.path_content_str().unwrap_or_default().to_owned();
        self.completion
            .complete(&self.input.string(), &self.path_content, &current_path)
    }

    /// Refresh the current view.
    /// Input string is emptied, the files are read again, the window of
    /// displayed files is reset.
    /// The first file is selected.
    pub fn refresh_view(&mut self) -> FmResult<()> {
        self.filter = FilterKind::All;
        self.input.reset();
        self.path_content
            .reset_files(&self.filter, self.show_hidden)?;
        self.window.reset(self.path_content.content.len());
        self.preview = Preview::new_empty();
        self.completion.reset();
        self.directory.clear();
        Ok(())
    }

    /// Move to the currently selected directory.
    /// Fail silently if the current directory is empty or if the selected
    /// file isn't a directory.
    pub fn go_to_child(&mut self) -> FmResult<()> {
        let childpath = &self
            .path_content
            .selected()
            .ok_or_else(|| FmError::custom("go_to_child", "Empty directory"))?
            .path
            .clone();
        self.history.push(childpath);
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
    pub fn set_pathcontent(&mut self, path: &path::Path) -> FmResult<()> {
        self.history.push(path);
        self.path_content
            .change_directory(path, &self.filter, self.show_hidden)?;
        self.window.reset(self.path_content.content.len());
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

    /// Returns the correct index jump target to a flagged files.
    pub fn find_jump_index(&self, jump_target: &path::Path) -> Option<usize> {
        self.path_content
            .content
            .iter()
            .position(|file| file.path == jump_target)
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
    pub fn refresh_users(&mut self, users_cache: Rc<UsersCache>) -> FmResult<()> {
        self.path_content
            .refresh_users(users_cache, &self.filter, self.show_hidden)
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
    pub fn tree_select_root(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_root(colors)
    }

    /// Move to the parent of current path
    pub fn move_to_parent(&mut self) -> FmResult<()> {
        let path = self.path_content.path.clone();
        if let Some(parent) = path.parent() {
            self.set_pathcontent(parent)?;
        }
        Ok(())
    }

    /// Select the parent of current node.
    /// If we were at the root node, move to the parent and make a new tree.
    pub fn tree_select_parent(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        if self.directory.tree.position.len() <= 1 {
            self.move_to_parent()?;
            self.make_tree(colors)?
        }
        self.directory.select_parent(colors)
    }

    /// Select the next sibling.
    pub fn tree_select_next(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_next(colors)
    }

    /// Select the previous siblging
    pub fn tree_select_prev(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_prev(colors)
    }

    /// Select the first child if any.
    pub fn tree_select_first_child(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_first_child(colors)
    }

    /// Go to the last leaf.
    pub fn tree_go_to_bottom_leaf(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.go_to_bottom_leaf(colors)
    }

    /// Returns the current path.
    /// It Tree mode, it's the path of the selected node.
    /// Else, it's the current path of pathcontent.
    pub fn current_path(&mut self) -> &path::Path {
        match self.mode {
            Mode::Tree => &self.directory.tree.current_node.fileinfo.path,
            _ => &self.path_content.path,
        }
    }

    /// Returns the directory owning the selected file.
    /// In Tree mode, it's the current directory if the selected node is a directory,
    /// its parent otherwise.
    /// In normal mode it's the current working directory.
    pub fn directory_of_selected(&self) -> FmResult<&path::Path> {
        match self.mode {
            Mode::Tree => {
                let fileinfo = &self.directory.tree.current_node.fileinfo;
                match fileinfo.file_kind {
                    FileKind::Directory => Ok(&self.directory.tree.current_node.fileinfo.path),
                    _ => Ok(fileinfo.path.parent().ok_or_else(|| {
                        FmError::custom("path of selected", "selected file should have a parent")
                    })?),
                }
            }
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
    pub fn make_tree(&mut self, colors: &Colors) -> FmResult<()> {
        let path = self.path_content.path.clone();
        let users_cache = &self.path_content.users_cache;
        self.directory =
            Directory::new(&path, users_cache, colors, &self.filter, self.show_hidden)?;
        Ok(())
    }

    pub fn set_mode(&mut self, new_mode: Mode) {
        self.previous_mode = self.mode;
        self.mode = new_mode;
    }

    pub fn reset_mode(&mut self) {
        self.mode = self.previous_mode;
        self.previous_mode = Mode::Normal;
    }

    pub fn need_second_window(&self) -> bool {
        !matches!(self.mode, Mode::Normal | Mode::Tree | Mode::Preview)
    }
}
