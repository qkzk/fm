use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;
use indicatif::InMemoryTerm;
use sysinfo::Disks;
use tuikit::term::Term;

use crate::common::is_program_in_path;
use crate::common::NVIM;
use crate::common::SS;
use crate::io::execute_and_output;
use crate::io::Args;
use crate::io::Opener;

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
    /// terminal
    pub term: Arc<Term>,
    /// Info about the running machine. Only used to detect disks
    /// and their mount points.
    pub disks: Disks,
    /// true if the application was launched inside a neovim terminal emulator
    pub inside_neovim: bool,
    /// queue of pairs (sources, dest) to be copied.
    /// it shouldn't be massive under normal usage so we can use a vector instead of an efficient queue data structure.
    pub copy_file_queue: Vec<(Vec<std::path::PathBuf>, std::path::PathBuf)>,
    pub in_mem_progress: Option<InMemoryTerm>,
}

impl InternalSettings {
    pub fn new(opener: Opener, term: Arc<Term>, disks: Disks) -> Self {
        let args = Args::parse();
        let force_clear = false;
        let must_quit = false;
        let nvim_server = args.server.clone();
        let inside_neovim = args.neovim;
        let copy_file_queue = vec![];
        let copy_progress = None;
        Self {
            force_clear,
            must_quit,
            nvim_server,
            opener,
            disks,
            term,
            inside_neovim,
            copy_file_queue,
            in_mem_progress: copy_progress,
        }
    }

    /// Returns the sice of the terminal (width, height)
    pub fn term_size(&self) -> Result<(usize, usize)> {
        Ok(self.term.term_size()?)
    }

    /// Set a "force clear" flag to true, which will reset the display.
    /// It's used when some command or whatever may pollute the terminal.
    /// We ensure to clear it before displaying again.
    pub fn force_clear(&mut self) {
        self.force_clear = true;
    }

    pub fn mount_points(&mut self) -> Vec<&std::path::Path> {
        self.disks.refresh_list();
        self.disks.iter().map(|d| d.mount_point()).collect()
    }

    pub fn update_nvim_listen_address(&mut self) {
        if let Ok(nvim_listen_address) = std::env::var("NVIM_LISTEN_ADDRESS") {
            self.nvim_server = nvim_listen_address;
        } else if let Ok(nvim_listen_address) = Self::parse_nvim_address_from_ss_output() {
            self.nvim_server = nvim_listen_address;
        }
    }

    fn parse_nvim_address_from_ss_output() -> Result<String> {
        if !is_program_in_path(SS) {
            return Err(anyhow!("{SS} isn't installed"));
        }
        if let Ok(output) = execute_and_output(SS, ["-l"]) {
            let output = String::from_utf8(output.stdout).unwrap_or_default();
            let content: String = output
                .split(&['\n', '\t', ' '])
                .filter(|w| w.contains(NVIM))
                .collect();
            if !content.is_empty() {
                return Ok(content);
            }
        }
        Err(anyhow!("Couldn't get nvim listen address from `ss` output"))
    }

    /// Remove the top of the copy queue.
    pub fn file_copied(&mut self) -> Result<()> {
        if self.copy_file_queue.is_empty() {
            Err(anyhow!("Copy File Pool is empty"))
        } else {
            self.copy_file_queue.remove(0);
            Ok(())
        }
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
}
