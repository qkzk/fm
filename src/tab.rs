use std::path;

use crate::args::Args;
use crate::completion::Completion;
use crate::config::Config;
use crate::content_window::ContentWindow;
use crate::fileinfo::PathContent;
use crate::fm_error::{FmError, FmResult};
use crate::input::Input;
use crate::last_edition::LastEdition;
use crate::mode::Mode;
use crate::opener::{load_opener, Opener};
use crate::preview::Preview;
use crate::shortcut::Shortcut;
use crate::visited::History;

static OPENER_PATH: &str = "~/.config/fm/opener.yaml";

/// Holds every thing about the current tab of the application.
/// Is responsible to execute commands depending on received events, mutating
/// the tab of the application.
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
    /// Configurable terminal executable
    pub terminal: String,
    /// Completion list and index in it.
    pub completion: Completion,
    /// Last edition command kind received
    pub last_edition: LastEdition,
    pub must_quit: bool,
    /// Lines of the previewed files.
    /// Empty if not in preview mode.
    pub preview: Preview,
    /// Visited directories
    pub history: History,
    /// Predefined shortcuts
    pub shortcut: Shortcut,
    pub opener: Opener,
    pub help: String,
}

impl Tab {
    /// Creates a new tab from args, config and height.
    pub fn new(args: Args, config: Config, height: usize, help: String) -> FmResult<Self> {
        let path = std::fs::canonicalize(path::Path::new(&args.path))?;
        let path_content = PathContent::new(path.clone(), args.all)?;
        let show_hidden = args.all;
        let nvim_server = args.server;
        let terminal = config.terminal;
        let mode = Mode::Normal;
        let line_index = 0;
        let window = ContentWindow::new(path_content.files.len(), height);
        let input = Input::default();
        let completion = Completion::default();
        let last_edition = LastEdition::Nothing;
        let must_quit = false;
        let preview = Preview::Empty;
        let mut history = History::default();
        history.push(&path);
        let shortcut = Shortcut::new();
        let opener = load_opener(OPENER_PATH, terminal.clone())
            .unwrap_or_else(|_| Opener::new(terminal.clone()));
        Ok(Self {
            mode,
            line_index,
            window,
            input,
            path_content,
            height,
            show_hidden,
            nvim_server,
            terminal,
            completion,
            last_edition,
            must_quit,
            preview,
            history,
            shortcut,
            opener,
            help,
        })
    }

    pub fn fill_completion(&mut self) -> FmResult<()> {
        match self.mode {
            Mode::Goto => self.completion.goto(&self.input.string),
            Mode::Exec => self.completion.exec(&self.input.string),
            Mode::Search => self
                .completion
                .search(&self.input.string, &self.path_content),
            _ => Ok(()),
        }
    }

    pub fn refresh_view(&mut self) -> FmResult<()> {
        self.line_index = 0;
        self.input.reset();
        self.path_content.reset_files()?;
        self.window.reset(self.path_content.files.len());
        Ok(())
    }

    pub fn go_to_child(&mut self) -> FmResult<()> {
        let childpath = self
            .path_content
            .selected_file()
            .ok_or_else(|| FmError::new("Empty directory"))?
            .path
            .clone();
        self.history.push(&childpath);
        self.path_content = PathContent::new(childpath, self.show_hidden)?;
        self.window.reset(self.path_content.files.len());
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

    pub fn path_str(&self) -> Option<String> {
        Some(self.path_content.path.to_str()?.to_owned())
    }
    pub fn set_pathcontent(&mut self, path: path::PathBuf) -> FmResult<()> {
        self.history.push(&path);
        self.path_content = PathContent::new(path, self.show_hidden)?;
        self.window.reset(self.path_content.files.len());
        Ok(())
    }
}
