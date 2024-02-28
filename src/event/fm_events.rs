use tuikit::event::Event;

/// Internal and terminal events.
/// Most of events are sent from the terminal emulator.
/// Here we wrap them with a few internal variants.
/// It allows us to capture all events at the same place and force some actions internally.
pub enum FmEvents {
    /// A refresh is required
    Refresh,
    /// User has saved its filenames and we can rename/create them
    BulkExecute,
    /// Event from the terminal itself (restart, resize, key, mouse etc.)
    Event(Event),
}
