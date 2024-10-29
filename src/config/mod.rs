//! Everything about configuration from text files in `$HOME/.config/fm`.
//!
//! - [`cloud_config::cloud_config`] is the function used to create a valid token for cloud files (google drive only ATM),
//! - `colors` holds everything about reading, parsing, converting & generating colors,
//! - `configuration`]holds everything about the yaml files used and their configuration,
//! - [`gradient::Gradient`] is the only color thing placed elsewhere, it's just a gradient generator of 254 variants from a static start to a static end. Doing so allows us to export those colors as a static array at runtime.
//! - `oncelock_static` holds all the static files configured by config and argument parameters. Those static files are set once from config and can be read every where in the application.
//! - [`keybindings::Bindings`] & [`keybindings::from_keyname`] are used to handle the user configured keybinds. The first creates default (hardcoded) keybinds and tries to update from from the config file, the second is used to read those configuration files.

mod cloud_config;
mod colors;
mod configuration;
mod gradient;
mod keybindings;
mod oncelock_static;

pub use cloud_config::cloud_config;
pub use colors::{extension_color, str_to_ratatui, ColorG, NormalFileColorer, MAX_GRADIENT_NORMAL};
pub use configuration::{load_config, read_normal_file_colorer, Config, FileStyle, MenuStyle};
pub use gradient::Gradient;
pub use keybindings::{from_keyname, Bindings};
pub use oncelock_static::{
    set_configurable_static, ARRAY_GRADIENT, COLORER, FILE_STYLES, MENU_STYLES, MONOKAI_THEME,
    START_FOLDER,
};
