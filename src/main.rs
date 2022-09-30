extern crate shellexpand;

use clap::Parser;
use tuikit::event::{Event, Key};
use tuikit::key::MouseButton;
use tuikit::term::{Term, TermHeight};

use fm::args::Args;
use fm::config::load_config;
use fm::display::Display;
use fm::status::Status;

static CONFIG_PATH: &str = "~/.config/fm/config.yaml";

fn main() {
    let args = Args::parse();
    eprintln!("Clap args {:?}", args);
    let term: Term<()> = Term::with_height(TermHeight::Percent(100)).unwrap();
    let _ = term.enable_mouse_support();
    let (_, height) = term.term_size().unwrap();
    let config = load_config(CONFIG_PATH);
    let mut status = Status::new(args, config, height);
    let mut display = Display::new(term);
    while let Ok(ev) = display.term.poll_event() {
        let _ = display.term.clear();
        let (_width, height) = display.term.term_size().unwrap();
        status.set_height(height);
        eprintln!("{:?}", ev);
        match ev {
            Event::Key(Key::ESC) => status.event_esc(),
            Event::Key(Key::Up) => status.event_up(),
            Event::Key(Key::Down) => status.event_down(),
            Event::Key(Key::Left) => status.event_left(),
            Event::Key(Key::Right) => status.event_right(),
            Event::Key(Key::Backspace) => status.event_backspace(),
            Event::Key(Key::Ctrl('d')) => status.event_delete_char(),
            Event::Key(Key::Delete) => status.event_delete_char(),
            Event::Key(Key::Char(c)) => status.event_char(c),
            Event::Key(Key::Home) => status.event_home(),
            Event::Key(Key::End) => status.event_end(),
            Event::Key(Key::PageDown) => status.event_page_down(),
            Event::Key(Key::PageUp) => status.event_page_up(),
            Event::Key(Key::Enter) => status.event_enter(),
            Event::Key(Key::Tab) => status.event_complete(),
            Event::Key(Key::WheelUp(_, _, _)) => status.event_up(),
            Event::Key(Key::WheelDown(_, _, _)) => status.event_down(),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => status.event_left_click(row),
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                status.event_right_click(row)
            }
            _ => {}
        }

        display.first_line(&status);
        display.files(&status);
        display.help_or_cursor(&status);
        display.jump_list(&status);
        display.completion(&status);

        let _ = display.term.present();
    }
}
