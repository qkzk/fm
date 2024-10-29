use crossterm::event::Event;

use crate::event::ActionMap;

/// Internal and terminal events.
/// Most of events are sent from the terminal emulator.
/// Here we wrap them with a few internal variants.
/// It allows us to capture all events at the same place and force some actions internally.
pub enum FmEvents {
    /// A refresh is required
    Refresh,
    /// User has saved its filenames and we can rename/create them
    BulkExecute,
    /// The first file in file queue has been copied
    FileCopied,
    /// Event from the terminal itself (restart, resize, key, mouse etc.)
    Term(Event),
    /// Action sent directly to be dispatched and executed
    Action(ActionMap),
    /// Empty events. Used to:
    /// - to check if a new preview should be attached
    /// - to send a "tick" to the fuzzy matcher if it's set
    UpdateTick,
}
