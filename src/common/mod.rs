//! Utiliy functions & constants.
//!
//! - `utils` holds a few functions which are used at various places, mostly to convert element from a type to another or to refresh states.
//! - `constant_strings_paths` holds every static string used to define paths and static messages (helper menu etc.)
//! - [`format::PathShortener`] is a trait allowing to shorten a path and display it in any width. Inspired by ranger.
//! - `random` holds everything about "pseudo" random generators. We don't really need randomness to create temporary files and a gradient of colors. A quick pseudo random does the trick.

mod constant_strings_paths;
mod format;
mod random;
mod utils;

pub use constant_strings_paths::*;
pub use format::*;
pub use random::random_alpha_chars;
pub use utils::*;
