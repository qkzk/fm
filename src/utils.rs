use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;

use sysinfo::{Disk, DiskExt};
use tuikit::term::Term;

use crate::event_dispatch::EventDispatcher;
use crate::fileinfo::human_size;
use crate::fm_error::FmResult;
use crate::status::Status;
use crate::term_manager::{Display, EventReader};

/// Returns a `Display` instance after `tuikit::term::Term` creation.
pub fn init_term() -> FmResult<Term> {
    let term: Term<()> = Term::new()?;
    term.enable_mouse_support()?;
    Ok(term)
}

fn disk_used_by_path<'a>(disks: &'a [Disk], path: &Path) -> Option<&'a Disk> {
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
pub fn print_on_quit(path_string: String) {
    println!("{}", path_string)
}

pub fn read_lines<P>(
    filename: P,
) -> std::io::Result<std::io::Lines<std::io::BufReader<std::fs::File>>>
where
    P: AsRef<std::path::Path>,
{
    let file = std::fs::File::open(filename)?;
    Ok(std::io::BufReader::new(file).lines())
}
