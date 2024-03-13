mod colors;
mod configuration;
mod keybindings;

pub use colors::{extension_color, ColorG, Colorer, Gradient};
pub use configuration::{
    load_config, Config, COLORER, COLORS, END_COLOR, MENU_COLORS, START_COLOR, START_FOLDER,
};
pub use keybindings::Bindings;
