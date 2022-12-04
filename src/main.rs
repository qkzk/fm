use std::sync::Arc;

use clap::Parser;
use log::info;
use sysinfo::{System, SystemExt};

use fm::actioner::Actioner;
use fm::args::Args;
use fm::config::load_config;
use fm::fm_error::FmResult;
use fm::help::Help;
use fm::log::set_logger;
use fm::status::Status;
use fm::term_manager::{Display, EventReader};
use fm::utils::{disk_space, init_term, print_on_quit};

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

/// Main function
/// Init the status and display and listen to events from keyboard and mouse.
/// The application is redrawn after every event.
fn main() -> FmResult<()> {
    set_logger()?;
    info!("fm is starting...");

    let config = load_config(CONFIG_PATH)?;

    let term = Arc::new(init_term()?);
    let actioner = Actioner::new(config.keybindings.clone());
    let event_reader = EventReader::new(term.clone());
    let help = Help::from_keybindings(&config.keybindings)?.help;
    let mut display = Display::new(term.clone(), config.colors.clone());
    let mut sys = System::new_all();
    let mut status = Status::new(Args::parse(), config, display.height()?, term.clone(), help)?;

    while let Ok(event) = event_reader.poll_event() {
        sys.refresh_disks();
        actioner.read_event(&mut status, event)?;
        display.display_all(
            &status,
            disk_space(sys.disks(), status.tabs[0].path_str().unwrap_or_default()),
            disk_space(sys.disks(), status.tabs[1].path_str().unwrap_or_default()),
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
