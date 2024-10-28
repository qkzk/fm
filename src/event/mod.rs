//! The events and their handling.
//!
//! Since we want to be able to configure keybinds, we can't use them directly in the code. So the architecture works like this:
//!
//! A **keybind** is mapped to an **Action** (from an enum) and this **Action** is then mapped to a method which can mutate the status.
//! A few more events are needed to refresh the filetree, stop the long running thread, attach built previews etc.
//! - [`action_map::ActionMap`] the actions themselve. From "Tab" (the user pressed tab) to custom events defined by the user.
//! - [`event_action::EventAction`] an empty struct which serves like an interface between events and status. All actions should be linked to a similarly named method there which take a mutable reference to the status, keybindings (if needed) and returns an empty `Result`. I couldn't find a way to enforce it at compile time but it's a strict guideline.
//! - [`event_dispatch::EventDispatcher`] the dispatcher of events. Mostly an interface used when an event is read and parse it according to the state (which window have focus etc.),
//! - [`event_poller::EventReader`] the reader of events which combine terminal events (from crossterm) and internal events (refresh the filetree, attach this preview, tick the fuzzyfinder etc.)
//! - [`fm_events::FmEvents`] those events. Terminal and internal events are combined here.

mod action_map;
mod event_action;
mod event_dispatch;
mod event_poller;
mod fm_events;

pub use action_map::ActionMap;
pub use event_action::EventAction;
pub use event_dispatch::EventDispatcher;
pub use event_poller::EventReader;
pub use fm_events::FmEvents;
