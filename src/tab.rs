use std::path;

use crate::args::Args;
use crate::completion::Completion;
use crate::content_window::ContentWindow;
use crate::fileinfo::PathContent;
use crate::filter::FilterKind;
use crate::fm_error::{FmError, FmResult};
use crate::indexed_vector::IndexedVector;
use crate::input::Input;
use crate::mode::Mode;
use crate::preview::Preview;
use crate::shortcut::Shortcut;
use crate::visited::History;

/// Holds every thing about the current tab of the application.
/// Most of the mutation is done externally.
#[derive(Clone)]
pub struct Tab {
    /// The mode the application is currenty in
    pub mode: Mode,
    /// The given index of a file.
    pub line_index: usize,
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
}

impl Tab {
    /// Creates a new tab from args and height.
    pub fn new(args: Args, height: usize) -> FmResult<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let path_content = PathContent::new(path.clone(), false)?;
        let show_hidden = false;
        let nvim_server = args.server;
        let mode = Mode::Normal;
        let line_index = 0;
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
            line_index,
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
        })
    }

    /// Fill the input string with the currently selected completion.
    pub fn fill_completion(&mut self) -> FmResult<()> {
        self.completion.set_kind(&self.mode);
        self.completion.complete(
            &self.input.string(),
            &self.path_content,
            self.path_str().unwrap_or_default(),
        )
    }

    /// Refresh the current view.
    /// Input string is emptied, the files are read again, the window of
    /// displayed files is reset.
    /// The first file is selected.
    pub fn refresh_view(&mut self) -> FmResult<()> {
        self.path_content.filter = FilterKind::All;
        self.line_index = 0;
        self.input.reset();
        self.path_content.reset_files()?;
        self.window.reset(self.path_content.content.len());
        Ok(())
    }

    /// Move to the currently selected directory.
    /// Fail silently if the current directory is empty or if the selected
    /// file isn't a directory.
    pub fn go_to_child(&mut self) -> FmResult<()> {
        let childpath = self
            .path_content
            .selected()
            .ok_or_else(|| FmError::custom("go_to_child", "Empty directory"))?
            .path
            .clone();
        self.history.push(&childpath);
        self.path_content = PathContent::new(childpath, self.show_hidden)?;
        self.window.reset(self.path_content.content.len());
        self.line_index = 0;
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
    pub fn path_str(&self) -> Option<String> {
        Some(self.path_content.path.to_str()?.to_owned())
    }

    /// Set the pathcontent to a new path.
    /// Reset the window.
    /// Add the last path to the history of visited paths.
    pub fn set_pathcontent(&mut self, path: path::PathBuf) -> FmResult<()> {
        self.history.push(&path);
        self.path_content.change_directory(&path)?;
        self.window.reset(self.path_content.content.len());
        self.line_index = 0;
        Ok(())
    }

    /// Set the window. Doesn't require the lenght to be known.
    pub fn set_window(&mut self) {
        let len = self.path_content.content.len();
        self.window.reset(len);
    }

    /// Set the line index to `index` and scroll there.
    pub fn scroll_to(&mut self, index: usize) {
        self.line_index = index;
        self.window.scroll_to(index);
    }

    /// Returns the correct index jump target to a flagged files.
    pub fn find_jump_index(&self, jump_target: &path::Path) -> Option<usize> {
        self.path_content
            .content
            .iter()
            .position(|file| file.path == jump_target)
    }

    /// Move the index one line up
    pub fn move_line_up(&mut self) {
        if self.line_index > 0 {
            self.line_index -= 1;
        }
    }

    /// Move the index one line down
    pub fn move_line_down(&mut self) {
        let max_line = self.path_content.content.len();
        if max_line >= ContentWindow::WINDOW_MARGIN_TOP
            && self.line_index < max_line - ContentWindow::WINDOW_MARGIN_TOP
        {
            self.line_index += 1;
        }
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh_shortcuts(&mut self, mount_points: &[&path::Path]) {
        self.shortcut.refresh(mount_points)
    }
}
