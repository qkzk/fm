mod cloud_config;
mod configurable_static;
mod configuration;
mod gradient;
mod keybindings;

pub use cloud_config::cloud_config;
pub use configurable_static::{
    set_configurable_static, COLORER, FILE_ATTRS, GRADIENT_NORMAL_FILE, MENU_ATTRS, MONOKAI_THEME,
    START_FOLDER,
};
pub use configuration::{
    load_color_from_config, load_config, parse_text_triplet, Config, FileAttr, MenuAttrs,
};
pub use gradient::{extension_color, ColorG, Gradient, NormalFileColorer};
pub use keybindings::Bindings;
