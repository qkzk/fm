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
    let final_path = status.selected_path_str();
    drop_everything(term, event_dispatcher, event_reader, status, display);
    print_on_quit(final_path);
    info!("fm is shutting down");
    Ok(())
}

fn main3() {
    use std::io;
    use tuikit::prelude::*;
    let chafa = std::fs::read_to_string("/home/quentin/Documents/8x8.png").unwrap();
    let term: Term<()> = Term::new().unwrap();
    let _ = term.print(3, 0, "term before pause");
    let _ = term.present();
    while let Ok(event) = term.poll_event() {
        match event {
            Event::Key(_) => {
                let _ = term.print(4, 0, "event before pause");
                let _ = term.present();
                break;
            }
            _ => (),
        }
    }
    let _ = term.pause();
    println!("{}", chafa);
    let mut buffer = String::new();
    let stdin = io::stdin(); // We get `Stdin` here.
    stdin.read_line(&mut buffer).unwrap();
    print!("\x1B[2J\x1B[1;1H");
    let _ = term.restart();
    let _ = term.clear();
    let _ = term.print(5, 0, "term after pause");
    let _ = term.present();
    while let Ok(event) = term.poll_event() {
        match event {
            Event::Key(_) => {
                break;
            }
            _ => (),
        }
    }
}
fn main4() {
    use std::io::{stdout, Write};
    use tuikit::raw::IntoRawMode;

    let mut stdout = stdout().into_raw_mode().unwrap();

    let text = std::fs::read_to_string("/home/quentin/chafa.txt").unwrap();

    write!(stdout, "{}", text).unwrap();
}
