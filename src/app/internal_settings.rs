use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc::Sender, Arc};

use anyhow::{bail, Result};
use clap::Parser;
use indicatif::InMemoryTerm;
use ratatui::layout::Size;
use sysinfo::Disks;

use crate::common::{is_in_path, open_in_current_neovim, NVIM, SS};
use crate::event::FmEvents;
use crate::io::{execute_and_output, Args, Extension, External, Opener};
use crate::modes::{copy_move, extract_extension, Content, Flagged};

/// Internal settings of the status.
///
/// Every setting which couldn't be attached elsewhere and is needed by the whole application.
/// It knows:
/// - if the content should be completely refreshed,
/// - if the application has to quit,
/// - the address of the nvim_server to send files to and if the application was launched from neovim,
/// - which opener should be used for kind of files,
/// - the height & width of the application,
/// - basic informations about disks being used,
/// - a copy queue to display informations about files beeing copied.
pub struct InternalSettings {
    /// Do we have to clear the screen ?
    pub force_clear: bool,
    /// True if the user issued a quit event (`Key::Char('q')` by default).
    /// It's used to exit the main loop before reseting the cursor.
    pub must_quit: bool,
    /// NVIM RPC server address
    pub nvim_server: String,
    /// The opener used by the application.
    pub opener: Opener,
    /// Termin size, width & height
    pub size: Size,
    /// Info about the running machine. Only used to detect disks
    /// and their mount points.
    pub disks: Disks,
    /// true if the application was launched inside a neovim terminal emulator
    pub inside_neovim: bool,
    /// queue of pairs (sources, dest) to be copied.
    /// it shouldn't be massive under normal usage so we can use a vector instead of an efficient queue data structure.
    pub copy_file_queue: Vec<(Vec<PathBuf>, PathBuf)>,
    /// internal progressbar used to display copy progress
    pub in_mem_progress: Option<InMemoryTerm>,
    /// true if the current terminal is disabled
    is_disabled: bool,
    /// true if the terminal should be cleared before exit. It's set to true when we reuse the window to start a new shell.
    pub clear_before_quit: bool,
}

impl InternalSettings {
    /// Creates a new instance. Some parameters (`nvim_server` and `inside_neovim`) are read from args.
    pub fn new(opener: Opener, size: Size, disks: Disks) -> Self {
        let args = Args::parse();
        let force_clear = false;
        let must_quit = false;
        let nvim_server = args.server.clone();
        let inside_neovim = args.neovim;
        let copy_file_queue = vec![];
        let in_mem_progress = None;
        let is_disabled = false;
        let clear_before_quit = false;
        Self {
            force_clear,
            must_quit,
            nvim_server,
            opener,
            disks,
            size,
            inside_neovim,
            copy_file_queue,
            in_mem_progress,
            is_disabled,
            clear_before_quit,
        }
    }

    #[inline]
    /// Returns the size of the terminal (width, height)
    pub fn term_size(&self) -> Size {
        self.size
    }

    /// Update the size from width & height.
    pub fn update_size(&mut self, width: u16, height: u16) {
        self.size = Size::from((width, height))
    }

    /// Set a "force clear" flag to true, which will reset the display.
    /// It's used when some command or whatever may pollute the terminal.
    /// We ensure to clear it before displaying again.
    pub fn force_clear(&mut self) {
        self.force_clear = true;
    }

    /// Reset the clear flag.
    /// Prevent the display from being completely reset for the next frame.
    pub fn reset_clear(&mut self) {
        self.force_clear = false;
    }

    /// True iff some event required a complete refresh of the dispplay
    pub fn should_be_cleared(&self) -> bool {
        self.force_clear
    }

    /// Refresh the disks -- removing non listed ones -- and returns a reference
    pub fn disks(&mut self) -> &Disks {
        self.disks.refresh(true);
        &self.disks
    }

    /// Returns a vector of mount points.
    /// Disks are refreshed first.
    pub fn mount_points_vec(&mut self) -> Vec<&Path> {
        self.disks().iter().map(|d| d.mount_point()).collect()
    }

    /// Returns a set of mount points
    pub fn mount_points_set(&self) -> HashSet<&Path> {
        self.disks
            .list()
            .iter()
            .map(|disk| disk.mount_point())
            .collect()
    }

    /// Tries its best to update the neovim address.
    /// 1. from the `$NVIM_LISTEN_ADDRESS` environment variable,
    /// 2. from the opened socket read from ss.
    ///
    /// # Warning
    /// If multiple neovim instances are opened at the same time, it will get the first one from the ss output.
    pub fn update_nvim_listen_address(&mut self) {
        if let Ok(nvim_listen_address) = std::env::var("NVIM_LISTEN_ADDRESS") {
            self.nvim_server = nvim_listen_address;
        } else if let Ok(nvim_listen_address) = Self::parse_nvim_address_from_ss_output() {
            self.nvim_server = nvim_listen_address;
        }
    }

    fn parse_nvim_address_from_ss_output() -> Result<String> {
        if !is_in_path(SS) {
            bail!("{SS} isn't installed");
        }
        if let Ok(output) = execute_and_output(SS, ["-l"]) {
            let output = String::from_utf8(output.stdout).unwrap_or_default();
            let content: String = output
                .split(&['\n', '\t', ' '])
                .find(|w| w.contains(NVIM))
                .unwrap_or("")
                .to_string();
            if !content.is_empty() {
                return Ok(content);
            }
        }
        bail!("Couldn't get nvim listen address from `ss` output")
    }

    /// Remove the top of the copy queue.
    pub fn copy_file_remove_head(&mut self) -> Result<()> {
        if !self.copy_file_queue.is_empty() {
            self.copy_file_queue.remove(0);
        }
        Ok(())
    }

    /// Start the copy of the next file in copy file queue and register the progress in
    /// the mock terminal used to create the display.
    pub fn copy_next_file_in_queue(
        &mut self,
        fm_sender: Arc<Sender<FmEvents>>,
        width: u16,
    ) -> Result<()> {
        let (sources, dest) = self.copy_file_queue[0].clone();
        let height = self.term_size().height;
        let in_mem = copy_move(
            crate::modes::CopyMove::Copy,
            sources,
            dest,
            width,
            height,
            fm_sender,
        )?;
        self.store_copy_progress(in_mem);
        Ok(())
    }

    /// Store copy progress bar.
    /// When a copy progress bar is stored,
    /// display manager is responsible for its display in the left tab.
    pub fn store_copy_progress(&mut self, in_mem_progress_bar: InMemoryTerm) {
        self.in_mem_progress = Some(in_mem_progress_bar);
    }

    /// Set copy progress bar to None.
    pub fn unset_copy_progress(&mut self) {
        self.in_mem_progress = None;
    }

    /// Disable the application display.
    /// It's used to give to allow another program to be executed.
    pub fn disable_display(&mut self) {
        self.is_disabled = true;
    }

    /// Display the application after it gave its terminal to another program.
    ///
    /// Enable the display again,
    /// clear the screen,
    /// set a flag to clear before quitting application.
    pub fn enable_display(&mut self) {
        if !self.is_disabled() {
            return;
        }
        self.is_disabled = false;
        self.force_clear();
        self.clear_before_quit = true;
    }

    /// True iff the terminal is disabled.
    /// The state (`self.is_disabled`) is changed every time
    /// a new shell is started replacing the normal window.
    /// If true, the display shouldn't be drawn.
    pub fn is_disabled(&self) -> bool {
        self.is_disabled
    }

    /// Open a new command which output will replace the current display.
    /// Current progress of the application is locked as long as the command doesn't finish.
    /// Firstly the display is disabled, then the command is ran.
    /// Once the command ends... the display is reenabled again.
    pub fn open_in_window<P>(&mut self, args: &[&str], current_path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.disable_display();
        External::open_command_in_window(args, current_path)?;
        self.enable_display();
        Ok(())
    }

    fn should_this_file_be_opened_in_neovim(&self, path: &Path) -> bool {
        matches!(Extension::matcher(extract_extension(path)), Extension::Text)
    }

    /// Open a single file:
    /// In neovim if this file should be,
    /// or in a new shell in current terminal,
    /// or in a new window.
    pub fn open_single_file<P>(&mut self, path: &Path, current_path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        if self.inside_neovim && self.should_this_file_be_opened_in_neovim(path) {
            self.update_nvim_listen_address();
            open_in_current_neovim(path, &self.nvim_server);
            Ok(())
        } else if self.opener.use_term(path) {
            self.open_single_in_window(path, current_path);
            Ok(())
        } else {
            self.opener.open_single(path)
        }
    }

    fn open_single_in_window<P>(&mut self, path: &Path, current_path: P)
    where
        P: AsRef<Path>,
    {
        self.disable_display();
        self.opener.open_in_window(path, current_path);
        self.enable_display();
    }

    /// Open all the flagged files.
    /// We try to open all files in a single command if it's possible.
    /// If all files should be opened in neovim, it will be.
    /// Otherwise, they will be opened separetely.
    pub fn open_flagged_files<P>(&mut self, flagged: &Flagged, current_path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        if self.inside_neovim && flagged.should_all_be_opened_in_neovim() {
            self.open_multiple_in_neovim(flagged.content());
            Ok(())
        } else {
            self.open_multiple_outside(flagged.content(), current_path)
        }
    }

    fn open_multiple_outside<P>(&mut self, paths: &[PathBuf], current_path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        let openers = self.opener.regroup_per_opener(paths);
        if Self::all_files_opened_in_terminal(&openers) {
            self.open_multiple_files_in_window(openers, current_path)
        } else {
            self.opener.open_multiple(openers)
        }
    }

    fn all_files_opened_in_terminal(openers: &HashMap<External, Vec<PathBuf>>) -> bool {
        openers.len() == 1 && openers.keys().next().expect("Can't be empty").use_term()
    }

    fn open_multiple_files_in_window<P>(
        &mut self,
        openers: HashMap<External, Vec<PathBuf>>,
        current_path: P,
    ) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.disable_display();
        self.opener.open_multiple_in_window(openers, current_path)?;
        self.enable_display();
        Ok(())
    }

    fn open_multiple_in_neovim(&mut self, paths: &[PathBuf]) {
        self.update_nvim_listen_address();
        for path in paths {
            open_in_current_neovim(path, &self.nvim_server);
        }
    }

    /// Set the must quit flag to true.
    /// The next update call will exit the application.
    /// It doesn't exit the application itself.
    pub fn quit(&mut self) {
        self.must_quit = true
    }

    /// Format the progress of the current operation in copy file queue.
    /// If nothing is being copied, it returns `None`
    pub fn format_copy_progress(&self) -> Option<String> {
        let Some(copy_progress) = &self.in_mem_progress else {
            return None;
        };
        let progress_bar = copy_progress.contents();
        let nb_copy_left = self.copy_file_queue.len();
        if nb_copy_left <= 1 {
            Some(progress_bar)
        } else {
            Some(format!(
                "{progress_bar}     -     1 of {nb}",
                nb = nb_copy_left
            ))
        }
    }
}
