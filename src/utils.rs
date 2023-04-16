use std::borrow::Borrow;
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use copypasta::{ClipboardContext, ClipboardProvider};
use sysinfo::{Disk, DiskExt};
use tuikit::term::Term;
use users::{get_current_uid, get_user_by_uid};

use crate::content_window::RESERVED_ROWS;
use crate::event_dispatch::EventDispatcher;
use crate::fileinfo::human_size;
use crate::nvim::nvim;
use crate::status::Status;
use crate::term_manager::{Display, EventReader};

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

/// Drops everything holding an `Arc<Term>`.
/// If new structs holding `Arc<Term>`  are introduced
/// (surelly to display something on their own...), we'll have to pass them
/// here and drop them.
/// It's used if the user wants to "cd on quit" which is a nice feature I
/// wanted to implement.
/// Since tuikit term redirects stdout, we have to drop them first.
pub fn drop_everything(
    term: Arc<Term>,
    event_dispatcher: EventDispatcher,
    event_reader: EventReader,
    status: Status,
    display: Display,
) {
    std::mem::drop(term);
    std::mem::drop(event_dispatcher);
    std::mem::drop(event_reader);
    std::mem::drop(status);
    std::mem::drop(display);
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

/// Get the current username as a String.
pub fn current_username() -> Result<String> {
    let user = get_user_by_uid(get_current_uid()).context("Couldn't read username")?;
    Ok(user
        .name()
        .to_str()
        .context("Couldn't read username")?
        .to_owned())
}

/// True iff the command is available in $PATH.
pub fn is_program_in_path(program: &str) -> bool {
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
    let Ok(mut ctx) = ClipboardContext::new() else { return Ok(()); };
    let Ok(_) = ctx.set_contents(content) else { return Ok(()); };
    // For some reason, it's not writen if you don't read it back...
    let _ = ctx.get_contents();
    Ok(())
}

pub fn row_to_index(row: u16) -> usize {
    row as usize - RESERVED_ROWS
}

pub fn string_to_path(path_string: &str) -> Result<std::path::PathBuf> {
    let expanded_cow_path = shellexpand::tilde(&path_string);
    let expanded_target: &str = expanded_cow_path.borrow();
    Ok(std::fs::canonicalize(expanded_target)?)
}

pub fn args_is_empty(args: &[String]) -> bool {
    args.is_empty() || args[0] == *""
}

pub fn is_sudo_command(executable: &str) -> bool {
    matches!(executable, "sudo")
}

pub fn open_in_current_neovim(path_str: &str, nvim_server: &str) {
    let command = &format!("<esc>:e {path_str}<cr><esc>:set number<cr><esc>:close<cr>");
    let _ = nvim(nvim_server, command);
}
