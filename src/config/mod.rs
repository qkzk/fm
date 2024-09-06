mod cloud_config;
mod colors;
mod configuration;
mod keybindings;

pub use cloud_config::cloud_config;
pub use colors::{extension_color, ColorG, Colorer, Gradient};
pub use configuration::{
    load_config, Config, COLORER, COLORS, MENU_COLORS, START_COLOR, START_FOLDER, STOP_COLOR,
};
pub use keybindings::Bindings;
