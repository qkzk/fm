use std::sync::Arc;

use anyhow::Result;
use log::info;

use fm::config::{load_config, Colors};
use fm::constant_strings_paths::{CONFIG_PATH, OPENER_PATH};
use fm::event_dispatch::EventDispatcher;
use fm::help::Help;
use fm::log::set_loggers;
use fm::opener::{load_opener, Opener};
use fm::status::Status;
use fm::term_manager::{Display, EventReader};
use fm::utils::{drop_everything, init_term, print_on_quit};

/// Exit the application and log a message.
/// Used when the config can't be read.
fn exit_wrong_config() -> ! {
    eprintln!("Couldn't load the config file at {CONFIG_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/config.yaml for an example.");
    info!("Couldn't read the config file {CONFIG_PATH}");
    std::process::exit(1)
}

/// Setup everything the application needs in its main loop :
/// an `EventReader`,
/// an `EventDispatcher`,
/// a `Status`,
/// a `Display`,
/// some `Colors`.
/// It reads and drops the configuration from the config file.
/// If the config can't be parsed, it exits with error code 1.
fn setup() -> Result<(
    Arc<tuikit::term::Term>,
    EventDispatcher,
    EventReader,
    Status,
    Display,
    Colors,
)> {
    let Ok(config) = load_config(CONFIG_PATH) else {
        exit_wrong_config()
    };
    let term = Arc::new(init_term()?);
    let event_dispatcher = EventDispatcher::new(config.binds.clone());
    let event_reader = EventReader::new(term.clone());
    let opener = load_opener(OPENER_PATH, &config.terminal).unwrap_or_else(|_| {
            eprintln!("Couldn't read the opener config file at {OPENER_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/opener.yaml for an example. Using default.");
            info!("Couldn't read opener file at {OPENER_PATH}. Using default.");
            Opener::new(&config.terminal)
        });
    let help = Help::from_keybindings(&config.binds, &opener)?.help;
    let display = Display::new(term.clone());
    let status = Status::new(
        display.height()?,
        term.clone(),
        help,
        opener,
        &config.settings,
    )?;
    let colors = config.colors.clone();
    drop(config);
    Ok((
        term,
        event_dispatcher,
        event_reader,
        status,
        display,
        colors,
    ))
}

/// Force clear the display if the status requires it, then reset it in status.
fn force_clear_if_needed(status: &mut Status, display: &mut Display) -> Result<()> {
    if status.force_clear {
        display.force_clear()?;
        status.force_clear = false;
    }
    Ok(())
}

/// Display the cursor,
/// drop everything holding a terminal instance,
/// print the final path
fn reset_and_print_on_quit(
    term: Arc<tuikit::term::Term>,
    event_dispatcher: EventDispatcher,
    event_reader: EventReader,
    status: Status,
    display: Display,
) -> Result<()> {
    display.show_cursor()?;
    let final_path = status.selected_path_str().to_owned();
    drop_everything(term, event_dispatcher, event_reader, status, display);
    print_on_quit(&final_path);
    info!("fm is shutting down");
    Ok(())
}

/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken and we reset the cursor.
fn main() -> Result<()> {
    set_loggers()?;
    info!("fm is starting");
    let (term, event_dispatcher, event_reader, mut status, mut display, colors) = setup()?;

    while let Ok(event) = event_reader.poll_event() {
        event_dispatcher.dispatch(&mut status, event, &colors, event_reader.term_height()?)?;
        status.refresh_disks();
        force_clear_if_needed(&mut status, &mut display)?;
        display.display_all(&status, &colors)?;

        if status.must_quit() {
            break;
        };
    }

    reset_and_print_on_quit(term, event_dispatcher, event_reader, status, display)
}
