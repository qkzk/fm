//! The different modes the application use. The application use 4 windows "display" & "menu", left & right.
//! Display holds the files or their content, menus regroup similar actions from changing permissions to establish an sshfs mount point.
//!
//! - [`fs::FileInfo`] is the internal representation of a file, an element of the filetree. It's constructed from its path and a hashmap of users. It holds the metadata, which are used to sort the files quickly.
//! - [`mode::Display`] and [`mode::Menu`] are the modes for top and bottom window of each tab (left or right). It's mostly an enum with a few common methods.
//! - [`display::Directory`], [`display::Tree`], [`display::Preview`] and [`display::FuzzyFinder`] are the 4 modes in which a main window can be. [`display::Ueber`] holds everything about files previewed with images.
//! - `menu` holds every kind of mode for the menu window. They can be separated into groups: input ? completion ? navigation ? require a confirmation ? Most of them are very basics. Actions to those menus are oftenly attached to status, since they require more information and change the state.
//! - [`utils::MenuHolder`] is the holder of those menus. For architectural reasons - which I regret now - we have to hold all those menus even if you never use them.
//! - `utils` holds also some common structs and traits which are used by those modes like the navigation, selection and drawing of a "content + index".

mod display;
mod fs;
mod menu;
mod mode;
mod utils;

pub use display::*;
pub use fs::*;
pub use menu::*;
pub use mode::{Display, InputSimple, Leave, MarkAction, Menu, Navigate, NeedConfirmation};
pub use utils::*;
