use std::sync::Arc;

use clap::Parser;
use log::info;
use sysinfo::{Disk, DiskExt, System, SystemExt};
use tuikit::term::Term;

use fm::actioner::Actioner;
use fm::args::Args;
use fm::config::load_config;
use fm::fileinfo::human_size;
use fm::fm_error::FmResult;
use fm::log::set_logger;
use fm::status::Status;
use fm::term_manager::{Display, EventReader};

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

/// Returns a `Display` instance after `tuikit::term::Term` creation.
fn init_term() -> FmResult<Term> {
    let term: Term<()> = Term::new()?;
    term.enable_mouse_support()?;
    Ok(term)
}

fn disk_space(disks: &[Disk], path_str: String) -> String {
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

fn print_on_quit(
    term: Arc<Term>,
    actioner: Actioner,
    event_reader: EventReader,
    status: Status,
    display: Display,
) {
    if status.print_path_on_quit {
        let path = status
            .selected_non_mut()
            .path_content
            .selected_path_str()
            .unwrap_or_default();
        std::mem::drop(term);
        std::mem::drop(actioner);
        std::mem::drop(event_reader);
        std::mem::drop(status);
        std::mem::drop(display);
        println!("{}", path)
    }
}

/// Main function
/// Init the status and display and listen to events from keyboard and mouse.
/// The application is redrawn after every event.
fn main() -> FmResult<()> {
    set_logger()?;
    info!("fm is starting...");

    let config = load_config(CONFIG_PATH);

    let term = Arc::new(init_term()?);
    let actioner = Actioner::new(&config.keybindings, term.clone());
    let event_reader = EventReader::new(term.clone());
    let mut display = Display::new(term.clone(), config.colors.clone());
    let mut sys = System::new_all();
    let mut status = Status::new(Args::parse(), config, display.height()?, term.clone())?;

    while let Ok(event) = event_reader.poll_event() {
        sys.refresh_disks();
        actioner.read_event(&mut status, event)?;
        display.display_all(
            &status,
            disk_space(
                sys.disks(),
                status.selected_non_mut().path_str().unwrap_or_default(),
            ),
        )?;

        if status.selected_non_mut().must_quit() {
            break;
        };
    }
    display.show_cursor()?;
    print_on_quit(term, actioner, event_reader, status, display);
    info!("fm is shutting down");
    Ok(())
}
