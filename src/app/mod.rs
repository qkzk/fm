//! The application and its status.
//!
//! We create here:
//! - [`application::FM`]: which run the application itself,
//! - [`displayer::Displayer`]: which displays the content at 30 fps,
//! - [`header_footer::ClickableLine`], [`header_footer::ClickableString`], [`header_footer::Footer`], [`header_footer::Header`], [`header_footer::PreviewHeader`]: a few structs to create clickable header & footer,
//! - [`internal_settings::InternalSettings`]: a bunch of settings for status,
//! - [`refresher::Refresher`]: looks for filetree changes and update of the fuzzy finder,
//! - [`session::Session`]: records basic updates in configuration (use double tab, preview in second tab, display metadata) to create a session for the user,
//! - [`status::Status`]: mutates most of the state of the application.
//! - [`status::Window`]: one of the 4 windows used (file left, menu left, file right, menu right)
//! - [`status::Focus`]: which of those windows have focus
//! - [`tab::Tab`]: responsible for the display of current directory & preview. The fuzzy finder is in status.

mod application;
mod displayer;
mod header_footer;
mod internal_settings;
mod previewer;
mod refresher;
mod session;
mod status;
mod tab;
mod thumbnailer;

pub use application::FM;
pub use displayer::Displayer;
pub use header_footer::{ClickableLine, ClickableString, Footer, Header, PreviewHeader};
pub use internal_settings::InternalSettings;
pub use previewer::{previewer_plugins, Previewer};
pub use refresher::Refresher;
pub use session::Session;
pub use status::{Direction, Focus, Status, Window};
pub use tab::{Tab, TabSettings};
pub use thumbnailer::ThumbnailManager;
