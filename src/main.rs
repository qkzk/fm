use clap::Parser;
use tuikit::term::Term;

use fm::actioner::Actioner;
use fm::args::Args;
use fm::config::load_config;
use fm::config::Colors;
use fm::display::Display;
use fm::status::Status;

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

/// Returns a `Display` instance after `tuikit::term::Term` creation.
fn init_display(colors: Colors) -> Display {
    let term: Term<()> = Term::new().unwrap();
    let _ = term.enable_mouse_support();
    Display::new(term, colors)
}

/// Display the cursor
fn reset_cursor(display: &Display) {
    let _ = display.term.show_cursor(true);
}

/// Main function.
/// Init the status and display and listen to events from keyboard and mouse.
/// The application is redrawn after every event.
fn main() {
    let config = load_config(CONFIG_PATH);
    let actioner = Actioner::new(&config.keybindings);
    let mut display = init_display(config.colors.clone());
    let mut status = Status::new(Args::parse(), config, display.height());

    while let Ok(event) = display.term.poll_event() {
        let _ = display.term.clear();
        let (_width, height) = display.term.term_size().unwrap();

        status.set_height(height);

        actioner.read_event(&mut status, event);

        display.display_all(&status);

        let _ = display.term.present();

        if status.must_quit() {
            reset_cursor(&display);
            break;
        };
    }
}
