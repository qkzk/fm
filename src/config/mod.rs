mod cloud_config;
mod colors;
mod configurable_static;
mod configuration;
mod keybindings;

pub use cloud_config::cloud_config;
pub use colors::{extension_color, ColorG, Colorer, Gradient};
pub use configurable_static::{
    set_configurable_static, COLORER, FILE_ATTRS, GRADIENT_NORMAL_FILE, MENU_ATTRS, MONOKAI_THEME,
    START_FOLDER,
};
pub use configuration::{load_color_from_config, load_config, Config, FileAttr, MenuAttrs};
pub use keybindings::Bindings;
