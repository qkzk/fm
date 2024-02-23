use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use tuikit::event::Event;

pub enum FmEvents {
    Refresh,
    BulkExecute,
    Event(Event),
}

pub fn init_events() -> (Sender<FmEvents>, Receiver<FmEvents>) {
    let (sender, receiver) = mpsc::channel::<FmEvents>();
    (sender, receiver)
}
