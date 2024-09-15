mod cloud_config;
mod colors;
mod configuration;
mod gradient;
mod keybindings;
mod oncelock_static;

pub use cloud_config::cloud_config;
pub use colors::{extension_color, str_to_tuikit, ColorG, NormalFileColorer};
pub use configuration::{load_config, read_normal_file_colorer, Config, FileAttr, MenuAttrs};
pub use gradient::Gradient;
pub use keybindings::Bindings;
pub use oncelock_static::{
    set_configurable_static, ARRAY_GRADIENT, COLORER, FILE_ATTRS, MENU_ATTRS, MONOKAI_THEME,
    START_FOLDER,
};
