use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tuikit::term::Term;

use crate::event::FmEvents;

/// Simple struct to read the events.
pub struct EventReader {
    pub term: Arc<Term>,
    pub fm_receiver: Receiver<FmEvents>,
}

impl EventReader {
    /// Creates a new instance with an Arc to a terminal.
    pub fn new(term: Arc<Term>, fm_receiver: Receiver<FmEvents>) -> Self {
        Self { term, fm_receiver }
    }

    /// Returns the events as they're received. Wait indefinitely for a new one.
    /// We should spend most of the application life here, doing nothing :)
    pub fn poll_event(&self) -> Result<FmEvents> {
        loop {
            if let Ok(event) = self.fm_receiver.try_recv() {
                return Ok(event);
            }
            if let Ok(event) = self.term.peek_event(Duration::from_millis(100)) {
                return Ok(FmEvents::Event(event));
            }
        }
    }
}
