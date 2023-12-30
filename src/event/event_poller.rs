use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tuikit::error::TuikitError;
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
    pub fn peek_event(&self) -> Result<Event, TuikitError> {
        self.term.peek_event(Duration::from_millis(17))
    }
}
