use std::sync::Arc;

use sysinfo::{Disk, DiskExt};
use tuikit::term::Term;

use crate::actioner::Actioner;
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

pub fn disk_space(disks: &[Disk], path_str: String) -> String {
    if path_str.is_empty() {
        return "".to_owned();
    }
    let mut size = 0_u64;
    let mut disks: Vec<&Disk> = disks.iter().collect();
    disks.sort_by_key(|disk| disk.mount_point().as_os_str().len());
    for disk in disks {
        if path_str.contains(disk.mount_point().as_os_str().to_str().unwrap()) {
            size = disk.available_space();
        };
    }
    human_size(size)
}

pub fn print_on_quit(
    term: Arc<Term>,
    actioner: Actioner,
    event_reader: EventReader,
    status: Status,
    display: Display,
) {
    let path = status.selected_non_mut().path_str().unwrap_or_default();
    std::mem::drop(term);
    std::mem::drop(actioner);
    std::mem::drop(event_reader);
    std::mem::drop(status);
    std::mem::drop(display);
    println!("{}", path)
}
