use std::sync::Arc;

use clap::Parser;
use log::info;

use fm::actioner::Actioner;
use fm::args::Args;
use fm::config::load_config;
use fm::fm_error::FmResult;
use fm::help::Help;
use fm::log::set_logger;
use fm::status::Status;
use fm::term_manager::{Display, EventReader};
use fm::utils::{init_term, print_on_quit};

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

/// Main function
/// Init the status and display and listen to events from keyboard and mouse.
/// The application is redrawn after every event.
fn main() -> FmResult<()> {
    set_logger()?;
    info!("fm is starting...");

    let config = load_config(CONFIG_PATH)?;
    let term = Arc::new(init_term()?);
    let actioner = Actioner::new(config.binds.clone());
    let event_reader = EventReader::new(term.clone());
    let help = Help::from_keybindings(&config.binds)?.help;
    let mut display = Display::new(term.clone(), config.colors.clone());
    let mut status = Status::new(Args::parse(), config, display.height()?, term.clone(), help)?;

    while let Ok(event) = event_reader.poll_event() {
        actioner.read_event(&mut status, event)?;
        status.refresh_disks();

        display.display_all(&status)?;

        if status.selected_non_mut().must_quit() {
            break;
        };
    }
    display.show_cursor()?;
    print_on_quit(term, actioner, event_reader, status, display);
    info!("fm is shutting down");
    Ok(())
}
