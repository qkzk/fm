mod cloud_config;
mod colors;
mod configuration;
mod keybindings;

pub use cloud_config::cloud_config;
pub use colors::{extension_color, ColorG, Colorer, Gradient};
pub use configuration::{
    load_color_from_config, load_config, Colors, Config, MenuColors, COLORER, COLORS,
    LAST_LOG_INFO, LAST_LOG_LINE, MENU_COLORS, MONOKAI_THEME, START_COLOR, START_FOLDER,
    STOP_COLOR,
};
pub use keybindings::Bindings;
