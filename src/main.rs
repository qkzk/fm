use clap::Parser;
use tuikit::term::{Term, TermHeight};

use fm::actioner::Actioner;
use fm::args::Args;
use fm::config::load_config;
use fm::display::Display;
use fm::status::Status;

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

/// Returns a `Display` instance after `tuikit::term::Term` creation.
fn init_display() -> Display {
    let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    let _ = term.enable_mouse_support();
    Display::new(term)
}

/// Main function.
/// Init the status and display and listen to events from keyboard and mouse.
/// The application is redrawn after every event.
fn main() {
    let config = load_config(CONFIG_PATH);
    let actioner = Actioner::new(&config.keybindings);
    let mut display = init_display();
    let mut status = Status::new(Args::parse(), config, display.height());

    while let Ok(event) = display.term.poll_event() {
        let _ = display.term.clear();
        let (_width, height) = display.term.term_size().unwrap();

        status.set_height(height);

        actioner.read_event(&mut status, event);

        display.display_all(&status);

        let _ = display.term.present();
    }
}
