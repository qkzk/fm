use tuikit::event::Event;

/// Internal and terminal events.
pub enum FmEvents {
    /// A refresh is required
    Refresh,
    /// User has saved its filenames and we can rename/create them
    BulkExecute,
    /// Event from the terminal itself (restart, resize, key, mouse etc.)
    Event(Event),
}
