use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc::Sender, Arc};

use anyhow::{anyhow, Result};
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
    /// terminal width
    pub width: u16,
    /// terminal height
    pub height: u16,
    /// Info about the running machine. Only used to detect disks
    /// and their mount points.
    // TODO: make it private and refactor disk space without using collect
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
    pub fn new(opener: Opener, size: Size, disks: Disks) -> Self {
        let args = Args::parse();
        let force_clear = false;
        let must_quit = false;
        let nvim_server = args.server.clone();
        let inside_neovim = args.neovim;
        let copy_file_queue = vec![];
        let in_mem_progress = None;
        let width = size.width;
        let height = size.height;
        let is_disabled = false;
        let clear_before_quit = false;
        Self {
            force_clear,
            must_quit,
            nvim_server,
            opener,
            disks,
            width,
            height,
            inside_neovim,
            copy_file_queue,
            in_mem_progress,
            is_disabled,
            clear_before_quit,
        }
    }

    // TODO: returns size
    /// Returns the size of the terminal (width, height)
    pub fn term_size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub fn update_size(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    /// Set a "force clear" flag to true, which will reset the display.
    /// It's used when some command or whatever may pollute the terminal.
    /// We ensure to clear it before displaying again.
    pub fn force_clear(&mut self) {
        self.force_clear = true;
    }

    pub fn reset_clear(&mut self) {
        self.force_clear = false;
    }

    pub fn should_be_cleared(&self) -> bool {
        self.force_clear
    }

    pub fn disks(&mut self) -> &Disks {
        self.disks.refresh_list();
        &self.disks
    }

    pub fn mount_points(&mut self) -> Vec<&Path> {
        self.disks().iter().map(|d| d.mount_point()).collect()
    }

    pub fn update_nvim_listen_address(&mut self) {
        if let Ok(nvim_listen_address) = std::env::var("NVIM_LISTEN_ADDRESS") {
            self.nvim_server = nvim_listen_address;
        } else if let Ok(nvim_listen_address) = Self::parse_nvim_address_from_ss_output() {
            self.nvim_server = nvim_listen_address;
        }
    }

    fn parse_nvim_address_from_ss_output() -> Result<String> {
        if !is_in_path(SS) {
            return Err(anyhow!("{SS} isn't installed"));
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
        Err(anyhow!("Couldn't get nvim listen address from `ss` output"))
    }

    /// Remove the top of the copy queue.
    pub fn copy_file_remove_head(&mut self) -> Result<()> {
        if self.copy_file_queue.is_empty() {
            Err(anyhow!("Copy File Pool is empty"))
        } else {
            self.copy_file_queue.remove(0);
            Ok(())
        }
    }

    pub fn copy_next_file_in_queue(
        &mut self,
        fm_sender: Arc<Sender<FmEvents>>,
        width: u16,
    ) -> Result<()> {
        let (sources, dest) = self.copy_file_queue[0].clone();
        let (_, height) = self.term_size();
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

    pub fn is_disabled(&self) -> bool {
        self.is_disabled
    }

    pub fn open_in_window(&mut self, args: &[&str]) -> Result<()> {
        self.disable_display();
        External::open_command_in_window(args)?;
        self.enable_display();
        Ok(())
    }

    fn should_this_file_be_opened_in_neovim(&self, path: &Path) -> bool {
        matches!(Extension::matcher(extract_extension(path)), Extension::Text)
    }

    pub fn open_single_file(&mut self, path: &Path) -> Result<()> {
        if self.inside_neovim && self.should_this_file_be_opened_in_neovim(path) {
            self.update_nvim_listen_address();
            open_in_current_neovim(path, &self.nvim_server);
            Ok(())
        } else if self.opener.use_term(path) {
            self.open_single_in_window(path);
            Ok(())
        } else {
            self.opener.open_single(path)
        }
    }

    fn open_single_in_window(&mut self, path: &Path) {
        self.disable_display();
        self.opener.open_in_window(path);
        self.enable_display();
    }

    pub fn open_flagged_files(&mut self, flagged: &Flagged) -> Result<()> {
        if self.inside_neovim && flagged.should_all_be_opened_in_neovim() {
            self.open_multiple_in_neovim(flagged.content());
            Ok(())
        } else {
            self.open_multiple_outside(flagged.content())
        }
    }

    fn open_multiple_outside(&mut self, paths: &[PathBuf]) -> Result<()> {
        let openers = self.opener.regroup_per_opener(paths);
        if Self::all_files_opened_in_terminal(&openers) {
            self.open_multiple_files_in_window(openers)
        } else {
            self.opener.open_multiple(openers)
        }
    }

    fn all_files_opened_in_terminal(openers: &HashMap<External, Vec<PathBuf>>) -> bool {
        openers.len() == 1 && openers.keys().next().expect("Can't be empty").use_term()
    }

    fn open_multiple_files_in_window(
        &mut self,
        openers: HashMap<External, Vec<PathBuf>>,
    ) -> Result<()> {
        self.disable_display();
        self.opener.open_multiple_in_window(openers)?;
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
