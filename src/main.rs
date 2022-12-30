use std::sync::Arc;

use clap::Parser;
use log::info;

use fm::args::Args;
use fm::config::load_config;
use fm::constant_strings_paths::CONFIG_PATH;
use fm::event_dispatch::EventDispatcher;
use fm::fm_error::FmResult;
use fm::help::Help;
use fm::log::set_logger;
use fm::status::Status;
use fm::term_manager::{Display, EventReader};
use fm::utils::{drop_everything, init_term, print_on_quit};

/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken and we reset the cursor.
fn main() -> FmResult<()> {
    set_logger()?;
    info!("fm is starting");

    let config = load_config(CONFIG_PATH)?;
    info!("config loaded");
    let term = Arc::new(init_term()?);
    let event_dispatcher = EventDispatcher::new(config.binds.clone());
    let event_reader = EventReader::new(term.clone());
    let help = Help::from_keybindings(&config.binds)?.help;
    let mut display = Display::new(term.clone(), config.colors.clone());
    let mut status = Status::new(Args::parse(), config, display.height()?, term.clone(), help)?;

    while let Ok(event) = event_reader.poll_event() {
        event_dispatcher.dispatch(&mut status, event)?;
        status.refresh_disks();
        display.display_all(&status)?;

        if status.selected_non_mut().must_quit() {
            break;
        };
    }

    display.show_cursor()?;
    let final_path = status.selected_path_str().to_owned();
    drop_everything(term, event_dispatcher, event_reader, status, display);
    print_on_quit(&final_path);
    info!("fm is shutting down");
    Ok(())
}
