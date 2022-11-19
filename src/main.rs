use std::sync::Arc;

use clap::Parser;
use log::info;
use tuikit::term::Term;

use fm::actioner::Actioner;
use fm::args::Args;
use fm::config::load_config;
use fm::display::Display;
use fm::fm_error::FmResult;
use fm::log::set_logger;
use fm::status::Status;

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

/// Returns a `Display` instance after `tuikit::term::Term` creation.
fn init_term() -> FmResult<Term> {
    let term: Term<()> = Term::new()?;
    term.enable_mouse_support()?;
    Ok(term)
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
    let mut display = Display::new(term.clone(), config.colors.clone());
    let mut status = Status::new(Args::parse(), config, display.height()?, term)?;

    while let Ok(event) = display.poll_event() {
        actioner.read_event(&mut status, event)?;
        display.display_all(&status)?;

        if status.selected_non_mut().must_quit() {
            break;
        };
    }
    display.show_cursor()?;
    info!("fm is shutting down");
    Ok(())
}
