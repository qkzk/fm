use std::borrow::Borrow;
use std::borrow::Cow;
use std::fs::metadata;
use std::io::BufRead;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result};
use copypasta::{ClipboardContext, ClipboardProvider};
use sysinfo::Disk;
use unicode_segmentation::UnicodeSegmentation;

use crate::common::CONFIG_FOLDER;
use crate::modes::{human_size, nvim, ContentWindow, Users};
use crate::{log_info, log_line};

/// Returns a `Display` instance after `tuikit::term::Term` creation.
// pub fn init_term() -> Result<Term> {
//     let term: Term<()> = Term::new()?;
//     term.enable_mouse_support()?;
//     Ok(term)
// }

/// Returns the disk owning a path.
/// None if the path can't be found.
///
/// We sort the disks by descending mount point size, then
/// we return the first disk whose mount point match the path.
pub fn disk_used_by_path<'a>(disks: &'a [&'a Disk], path: &Path) -> Option<&'a Disk> {
    let mut disks: Vec<&Disk> = disks.to_vec();
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
pub fn disk_space(disks: &[&Disk], path: &Path) -> String {
    if path.as_os_str().is_empty() {
        return "".to_owned();
    }
    disk_space_used(disk_used_by_path(disks, path))
}

/// Print the path on the stdout.
pub fn print_on_quit(path_string: String) {
    log_info!("print on quit {path_string}");
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
    Users::only_users()
        .get_user_by_uid(current_uid()?)
        .context("Couldn't read my own name")
        .cloned()
}

/// True if the program is given by an absolute path which exists or
/// if the command is available in $PATH.
pub fn is_in_path<S>(program: S) -> bool
where
    S: Into<String> + std::fmt::Display + AsRef<Path>,
{
    let p = program.to_string();
    let Some(program) = p.split_whitespace().next() else {
        return false;
    };
    if Path::new(program).exists() {
        return true;
    }
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

pub fn get_clipboard() -> Option<String> {
    let Ok(mut ctx) = ClipboardContext::new() else {
        return None;
    };
    ctx.get_contents().ok()
}

pub fn set_clipboard(content: String) {
    log_info!("copied to clipboard: {}", content);
    let Ok(mut ctx) = ClipboardContext::new() else {
        return;
    };
    let Ok(_) = ctx.set_contents(content) else {
        return;
    };
    // For some reason, it's not writen if you don't read it back...
    let _ = ctx.get_contents();
}

/// Copy the filename to the clipboard. Only the filename.
pub fn filename_to_clipboard(path: &std::path::Path) {
    let Some(filename) = path.file_name() else {
        return;
    };
    let filename = filename.to_string_lossy().to_string();
    set_clipboard(filename)
}

/// Copy the filepath to the clipboard. The absolute path.
pub fn filepath_to_clipboard(path: &std::path::Path) {
    let path = path.to_string_lossy().to_string();
    set_clipboard(path)
}

/// Convert a row into a `crate::fm::ContentWindow` index.
/// Just remove the header rows.
pub fn row_to_window_index(row: u16) -> usize {
    row as usize - ContentWindow::HEADER_ROWS
}

/// Convert a string into a valid, expanded and canonicalized path.
/// Doesn't check if the path exists.
pub fn string_to_path(path_string: &str) -> Result<std::path::PathBuf> {
    let expanded_cow_path = tilde(path_string);
    let expanded_target: &str = expanded_cow_path.borrow();
    Ok(std::fs::canonicalize(expanded_target)?)
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
    crate::common::random_alpha_chars()
        .take(7)
        .for_each(|ch| rand_str.push(ch));
    rand_str.push_str(".txt");
    rand_str
}

/// Clear the temporary file used by fm for previewing.
pub fn clear_tmp_files() {
    let Ok(read_dir) = std::fs::read_dir("/tmp") else {
        return;
    };
    read_dir
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("fm_thumbnail"))
        .for_each(|e| std::fs::remove_file(e.path()).unwrap_or_default())
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

/// True iff the last modification of given path happened less than `seconds` ago.
/// If the path has a modified time in future (ie. poorly configured iso file) it
/// will log an error and returns false.
pub fn has_last_modification_happened_less_than<P>(path: P, seconds: u64) -> Result<bool>
where
    P: AsRef<std::path::Path>,
{
    let modified = path.as_ref().metadata()?.modified()?;
    if let Ok(elapsed) = modified.elapsed() {
        let need_refresh = elapsed < std::time::Duration::new(seconds, 0);
        Ok(need_refresh)
    } else {
        let dt: chrono::DateTime<chrono::offset::Utc> = modified.into();
        let fmt = dt.format("%Y/%m/%d %T");
        log_info!(
            "Error for {path} modified datetime {fmt} is in future",
            path = path.as_ref().display(),
        );
        Ok(false)
    }
}

/// Rename a file giving it a new file name.
/// It uses `std::fs::rename` and `std::fs:create_dir_all` and has same limitations.
/// If the new name contains intermediate slash (`'/'`) like: `"a/b/d"`,
/// all intermediate folders will be created in the parent folder of `old_path` if needed.
///
/// # Errors
///
/// It may fail for the same reasons as [`std::fs::rename`] and [`std::fs::create_dir_all`].
/// See those for more details.
pub fn rename<P, Q>(old_path: P, new_name: Q) -> Result<std::path::PathBuf>
where
    P: AsRef<std::path::Path>,
    Q: AsRef<std::path::Path>,
{
    let Some(old_parent) = old_path.as_ref().parent() else {
        return Err(anyhow::anyhow!(
            "no parent for {old_path}",
            old_path = old_path.as_ref().display()
        ));
    };
    let new_path = old_parent.join(new_name);
    let Some(new_parent) = new_path.parent() else {
        return Err(anyhow::anyhow!(
            "no parent for {new_path}",
            new_path = new_path.display()
        ));
    };

    log_info!(
        "renaming: {} -> {}",
        old_path.as_ref().display(),
        new_path.display()
    );
    log_line!(
        "renaming: {} -> {}",
        old_path.as_ref().display(),
        new_path.display()
    );

    std::fs::create_dir_all(new_parent)?;
    std::fs::rename(old_path, &new_path)?;
    Ok(new_path)
}

/// This trait `UtfWidth` is defined with a single
/// method `utf_width()` that returns the width of
/// a string in Unicode code points.
/// The implementation for `String` and `&str`
/// types are provided which calculate the
/// width. of the original string in graphemes.
/// This method allows for easy calculation of
/// the horizontal space required for displaying
/// a given text, which can be useful for layout purposes.
pub trait UtfWidth {
    /// Number of graphemes in the string.
    /// Used to know the necessary width to print this text.
    fn utf_width(&self) -> usize;
    /// Number of graphemes in the string as a, u16.
    /// Used to know the necessary width to print this text.
    fn utf_width_u16(&self) -> u16;
}

impl UtfWidth for String {
    fn utf_width(&self) -> usize {
        self.as_str().utf_width()
    }

    fn utf_width_u16(&self) -> u16 {
        self.utf_width() as u16
    }
}

impl UtfWidth for &str {
    fn utf_width(&self) -> usize {
        self.graphemes(true)
            .map(|s| s.to_string())
            .collect::<Vec<String>>()
            .len()
    }

    fn utf_width_u16(&self) -> u16 {
        self.utf_width() as u16
    }
}

pub fn index_from_a(letter: char) -> Option<usize> {
    (letter as usize).checked_sub('a' as usize)
}

/// A PathBuf of the current config folder.
pub fn path_to_config_folder() -> Result<PathBuf> {
    Ok(std::path::PathBuf::from_str(tilde(CONFIG_FOLDER).borrow())?)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .and_then(|h| if h.is_empty() { None } else { Some(h) })
        .map(PathBuf::from)
}

/// Expand ~/Downloads to /home/user/Downloads where user is the current user.
/// Copied from <https://gitlab.com/ijackson/rust-shellexpand/-/blob/main/src/funcs.rs?ref_type=heads#L673>
pub fn tilde(input_str: &str) -> Cow<str> {
    if let Some(input_after_tilde) = input_str.strip_prefix('~') {
        if input_after_tilde.is_empty() || input_after_tilde.starts_with('/') {
            if let Some(hd) = home_dir() {
                let result = format!("{}{}", hd.display(), input_after_tilde);
                result.into()
            } else {
                // home dir is not available
                input_str.into()
            }
        } else {
            // we cannot handle `~otheruser/` paths yet
            input_str.into()
        }
    } else {
        // input doesn't start with tilde
        input_str.into()
    }
}
