use std::path;
use std::rc::Rc;

use users::UsersCache;

use crate::args::Args;
use crate::completion::Completion;
use crate::config::Colors;
use crate::content_window::ContentWindow;
use crate::fileinfo::PathContent;
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
    /// The indexes of displayed file
    pub window: ContentWindow,
    /// Files marked as flagged
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
}

impl Tab {
    /// Creates a new tab from args and height.
    pub fn new(args: Args, height: usize, users_cache: Rc<UsersCache>) -> FmResult<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let tree = Directory::empty(&path, &users_cache)?;
        let path_content = PathContent::new(&path, false, users_cache)?;
        let show_hidden = false;
        let nvim_server = args.server;
        let mode = Mode::Normal;
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
            window,
            input,
            path_content,
            height,
            show_hidden,
            nvim_server,
            completion,
            must_quit,
            preview,
            history,
            shortcut,
            searched,
            directory: tree,
        })
    }

    /// Fill the input string with the currently selected completion.
    pub fn fill_completion(&mut self) -> FmResult<()> {
        self.completion.set_kind(&self.mode);
        let current_path = self.path_str().unwrap_or_default().to_owned();
        self.completion
            .complete(&self.input.string(), &self.path_content, &current_path)
    }

    /// Refresh the current view.
    /// Input string is emptied, the files are read again, the window of
    /// displayed files is reset.
    /// The first file is selected.
    pub fn refresh_view(&mut self) -> FmResult<()> {
        self.path_content.filter = FilterKind::All;
        self.input.reset();
        self.path_content.reset_files()?;
        self.window.reset(self.path_content.content.len());
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
    pub fn path_str(&self) -> Option<&str> {
        self.path_content.path.to_str()
    }

    /// Set the pathcontent to a new path.
    /// Reset the window.
    /// Add the last path to the history of visited paths.
    pub fn set_pathcontent(&mut self, path: &path::Path) -> FmResult<()> {
        self.history.push(path);
        self.path_content.change_directory(path)?;
        self.window.reset(self.path_content.content.len());
        Ok(())
    }

    /// Set the window. Doesn't require the lenght to be known.
    pub fn set_window(&mut self) {
        let len = self.path_content.content.len();
        self.window.reset(len);
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

    pub fn go_to_index(&mut self, index: usize) {
        self.path_content.select_index(index);
        self.window.scroll_to(index);
    }

    /// Refresh the existing users.
    pub fn refresh_users(&mut self, users_cache: Rc<UsersCache>) -> FmResult<()> {
        self.path_content.refresh_users(users_cache)
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

    pub fn tree_select_root(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_root(colors)
    }

    pub fn tree_select_parent(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_parent(colors)
    }

    pub fn tree_select_next_sibling(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_next_sibling(colors)
    }

    pub fn tree_select_prev_sibling(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_prev_sibling(colors)
    }

    pub fn tree_select_first_child(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.select_first_child(colors)
    }

    pub fn tree_go_to_bottom_leaf(&mut self, colors: &Colors) -> FmResult<()> {
        self.directory.unselect_children();
        self.directory.go_to_bottom_leaf(colors)
    }
}
