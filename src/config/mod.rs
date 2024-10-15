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
pub use keybindings::Bindings;
pub use oncelock_static::{
    set_configurable_static, ARRAY_GRADIENT, COLORER, FILE_ATTRS, MENU_STYLES, MONOKAI_THEME,
    START_FOLDER,
};
