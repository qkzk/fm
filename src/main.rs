extern crate shellexpand;

use clap::Parser;
use tuikit::term::{Term, TermHeight};

use fm::args::Args;
use fm::config::load_config;
use fm::display::Display;
use fm::status::Status;

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

fn init_status(height: usize) -> Status {
    let args = Args::parse();
    let config = load_config(CONFIG_PATH);
    Status::new(args, config, height)
}

fn init_display() -> Display {
    let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    let _ = term.enable_mouse_support();
    Display::new(term)
}

fn main() {
    let mut display = init_display();
    let mut status = init_status(display.height());

    while let Ok(event) = display.term.poll_event() {
        let _ = display.term.clear();
        let (_width, height) = display.term.term_size().unwrap();

        status.set_height(height);
        status.read_event(event);

        display.display_all(&status);

        let _ = display.term.present();
    }
}
