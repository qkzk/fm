use std::borrow::Borrow;
use std::fs::metadata;
use std::io::BufRead;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{Context, Result};
use copypasta::{ClipboardContext, ClipboardProvider};
use rand::Rng;
use sysinfo::{Disk, DiskExt};
use tuikit::term::Term;

use crate::common::{CALC_PDF_PATH, THUMBNAIL_PATH};
use crate::display_mode::human_size;
use crate::display_mode::ContentWindow;
use crate::display_mode::Users;
use crate::edit_mode::nvim;
use crate::log_line;

/// Returns a `Display` instance after `tuikit::term::Term` creation.
pub fn init_term() -> Result<Term> {
    let term: Term<()> = Term::new()?;
    term.enable_mouse_support()?;
    Ok(term)
}

/// Returns the disk owning a path.
/// None if the path can't be found.
///
/// We sort the disks by descending mount point size, then
/// we return the first disk whose mount point match the path.
pub fn disk_used_by_path<'a>(disks: &'a [Disk], path: &Path) -> Option<&'a Disk> {
    let mut disks: Vec<&Disk> = disks.iter().collect();
    disks.sort_by_key(|disk| disk.mount_point().as_os_str().len());
    disks.reverse();
    disks
        .into_iter()
        .find(|&disk| path.starts_with(disk.mount_point()))
}

fn disk_space_used(disk: Option<&Disk>) -> String {
    match disk {
        None => "".to_owned(),
        Some(disk) => human_size(disk.available_space()),
    }
}

/// Returns the disk space of the disk holding this path.
/// We can't be sure what's the disk of a given path, so we have to look
/// if the mount point is a parent of given path.
/// This solution is ugly but... for a lack of a better one...
pub fn disk_space(disks: &[Disk], path: &Path) -> String {
    if path.as_os_str().is_empty() {
        return "".to_owned();
    }
    disk_space_used(disk_used_by_path(disks, path))
}

/// Print the path on the stdout.
pub fn print_on_quit(path_string: &str) {
    println!("{path_string}")
}

/// Returns the buffered lines from a text file.
pub fn read_lines<P>(
    filename: P,
) -> std::io::Result<std::io::Lines<std::io::BufReader<std::fs::File>>>
where
    P: AsRef<std::path::Path>,
{
    let file = std::fs::File::open(filename)?;
    Ok(std::io::BufReader::new(file).lines())
}

/// Extract a filename from a path reference.
/// May fail if the filename isn't utf-8 compliant.
pub fn filename_from_path(path: &std::path::Path) -> Result<&str> {
    path.file_name()
        .unwrap_or_default()
        .to_str()
        .context("couldn't parse the filename")
}

/// Uid of the current user.
/// Read from `/proc/self`.
/// Should never fail.
pub fn current_uid() -> Result<u32> {
    Ok(metadata("/proc/self").map(|metadata| metadata.uid())?)
}

/// Get the current username as a String.
/// Read from `/proc/self` and then `/etc/passwd` and should never fail.
pub fn current_username() -> Result<String> {
    Users::new()
        .get_user_by_uid(current_uid()?)
        .context("Couldn't read my own name")
}

/// True iff the command is available in $PATH.
pub fn is_program_in_path<S>(program: S) -> bool
where
    S: Into<String> + std::fmt::Display,
{
    if let Ok(path) = std::env::var("PATH") {
        for p in path.split(':') {
            let p_str = &format!("{p}/{program}");
            if std::path::Path::new(p_str).exists() {
                return true;
            }
        }
    }
    false
}

/// Extract the lines of a string
pub fn extract_lines(content: String) -> Vec<String> {
    content.lines().map(|line| line.to_string()).collect()
}

pub fn set_clipboard(content: String) -> Result<()> {
    log::info!("copied to clipboard: {}", content);
    let Ok(mut ctx) = ClipboardContext::new() else {
        return Ok(());
    };
    let Ok(_) = ctx.set_contents(content) else {
        return Ok(());
    };
    // For some reason, it's not writen if you don't read it back...
    let _ = ctx.get_contents();
    Ok(())
}

/// Convert a row into a `crate::fm::ContentWindow` index.
/// Just remove the header rows.
pub fn row_to_window_index(row: u16) -> usize {
    row as usize - ContentWindow::HEADER_ROWS
}

/// Convert a string into a valid, expanded and canonicalized path.
/// Doesn't check if the path exists.
pub fn string_to_path(path_string: &str) -> Result<std::path::PathBuf> {
    let expanded_cow_path = shellexpand::tilde(&path_string);
    let expanded_target: &str = expanded_cow_path.borrow();
    Ok(std::fs::canonicalize(expanded_target)?)
}

pub fn args_is_empty(args: &[String]) -> bool {
    args.is_empty() || args[0] == *""
}

/// True if the executable is "sudo"
pub fn is_sudo_command(executable: &str) -> bool {
    matches!(executable, "sudo")
}

/// Open the path in neovim.
pub fn open_in_current_neovim(path: &Path, nvim_server: &str) {
    let command = &format!(
        "<esc>:e {path}<cr><esc>:set number<cr><esc>:close<cr>",
        path = path.display()
    );
    match nvim(nvim_server, command) {
        Ok(()) => log_line!(
            "Opened {path} in neovim at {nvim_server}",
            path = path.display()
        ),
        Err(error) => log_line!(
            "Couldn't open {path} in neovim. Error {error:?}",
            path = path.display()
        ),
    }
}

/// Creates a random string.
/// The string starts with `fm-` and contains 7 random alphanumeric characters.
pub fn random_name() -> String {
    let mut rand_str = String::with_capacity(10);
    rand_str.push_str("fm-");
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(7)
        .for_each(|ch| rand_str.push(ch as char));
    rand_str.push_str(".txt");
    rand_str
}

/// Clear the temporary file used by fm for previewing.
pub fn clear_tmp_file() {
    let _ = std::fs::remove_file(THUMBNAIL_PATH);
    let _ = std::fs::remove_file(CALC_PDF_PATH);
}

/// True if the directory is empty,
/// False if it's not.
/// Err if the path doesn't exists or isn't accessible by
/// the user.
pub fn is_dir_empty(path: &std::path::Path) -> Result<bool> {
    Ok(path.read_dir()?.next().is_none())
}

pub fn path_to_string<P>(path: &P) -> String
where
    P: AsRef<std::path::Path>,
{
    path.as_ref().to_string_lossy().into_owned()
}
