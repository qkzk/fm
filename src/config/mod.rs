mod colors;
mod configuration;
mod keybindings;

pub use colors::{extension_color, Colorer};
pub use configuration::{load_config, Config, Settings, COLORER, COLORS};
pub use keybindings::{Bindings, REFRESH_EVENT};