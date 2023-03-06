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

    let Ok(config) = load_config(CONFIG_PATH) else {
        eprintln!("Couldn't load the config file at {CONFIG_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/config.yaml for an example.");
        info!("Couldn't read the config file {CONFIG_PATH}");
        std::process::exit(1);
    };
    info!("config loaded");
    let term = Arc::new(init_term()?);
    let event_dispatcher = EventDispatcher::new(config.binds.clone());
    let event_reader = EventReader::new(term.clone());
    let help = Help::from_keybindings(&config.binds)?.help;
    let mut display = Display::new(term.clone());
    let mut status = Status::new(
        Args::parse(),
        display.height()?,
        term.clone(),
        help,
        &config.terminal,
    )?;
    let colors = config.colors.clone();
    drop(config);

    while let Ok(event) = event_reader.poll_event() {
        event_dispatcher.dispatch(&mut status, event, &colors)?;
        status.refresh_disks();
        if status.force_clear {
            display.force_clear()?;
            status.force_clear = false;
        }
        display.display_all(&status, &colors)?;

        if status.must_quit() {
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
