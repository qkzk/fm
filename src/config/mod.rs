mod colors;
mod configuration;
mod keybindings;

pub use colors::{extension_color, Colorer};
pub use configuration::{load_config, Config, COLORER, COLORS, MENU_COLORS, START_FOLDER};
pub use keybindings::{Bindings, REFRESH_EVENT, REFRESH_KEY};
