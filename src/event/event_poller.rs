use std::sync::mpsc::Receiver;
use std::time::Duration;

use crate::event::FmEvents;

/// Simple struct to read the events.
pub struct EventReader {
    pub fm_receiver: Receiver<FmEvents>,
}

impl EventReader {
    /// Creates a new instance with an Arc to a terminal.
    pub fn new(fm_receiver: Receiver<FmEvents>) -> Self {
        Self { fm_receiver }
    }

    /// Returns the events as they're received. Loops until an event is received.
    /// We should spend most of the application life here, doing nothing :)
    ///
    /// It's an interface for internal and extenal events (through terminal)
    /// casting them into an [`crate::event::FmEvents`].
    pub fn poll_event(&self) -> FmEvents {
        loop {
            if let Ok(event) = self.fm_receiver.try_recv() {
                return event;
            }
            let Ok(true) = crossterm::event::poll(Duration::from_millis(20)) else {
                continue;
            };
            let Ok(term_event) = crossterm::event::read() else {
                continue;
            };
            return FmEvents::Term(term_event);
        }
    }
}
