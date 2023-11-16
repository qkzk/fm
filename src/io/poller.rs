use std::sync::Arc;

use anyhow::Result;
use tuikit::event::Event;
use tuikit::term::Term;

/// Simple struct to read the events.
pub struct EventReader {
    term: Arc<Term>,
}

impl EventReader {
    /// Creates a new instance with an Arc to a terminal.
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    /// Returns the events as they're received. Wait indefinitely for a new one.
    /// We should spend most of the application life here, doing nothing :)
    pub fn poll_event(&self) -> Result<Event> {
        Ok(self.term.poll_event()?)
    }

    /// Height of the current terminal
    pub fn term_height(&self) -> Result<usize> {
        Ok(self.term.term_size()?.1)
    }
}
