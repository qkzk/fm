use std::hash::Hasher;

use tuikit::attr::Color;

use crate::constant_strings_paths::CONFIG_PATH;

/// No attr but 3 static methods.
struct Colorer {}

impl Colorer {
    /// Picks a blueish/greenish color on color picker hexagon's perimeter.
    fn color_green_blue(hash: usize) -> Color {
        (128..255)
            .map(|b| Color::Rgb(0, 255, b))
            .chain((128..255).map(|g| Color::Rgb(0, g, 255)))
            .nth(hash % 254)
            .unwrap()
    }

    /// Picks a redish/blueish color on color picker hexagon's perimeter.
    fn color_red_blue(hash: usize) -> Color {
        (128..255)
            .map(|b| Color::Rgb(255, 0, b))
            .chain((128..255).map(|r| Color::Rgb(r, 0, 255)))
            .nth(hash % 254)
            .unwrap()
    }

    /// Picks a redish/greenish color on color picker hexagon's perimeter.
    fn color_red_green(hash: usize) -> Color {
        (128..255)
            .map(|r| Color::Rgb(r, 255, 0))
            .chain((128..255).map(|g| Color::Rgb(255, g, 0)))
            .nth(hash % 254)
            .unwrap()
    }
}

lazy_static::lazy_static! {
    /// Defines a palette which will color the "normal" files based on their extension.
    /// We try to read a yaml value and pick one of 3 palettes :
    /// "red-green", "red-blue" and "green-blue" which is the default.
    static ref COLORER: fn(usize) -> Color = {
        let mut colorer = Colorer::color_green_blue as fn(usize) -> Color;
        if let Ok(file) = std::fs::File::open(std::path::Path::new(&shellexpand::tilde(CONFIG_PATH).to_string())) {
            if let Ok(yaml)  = serde_yaml::from_reader::<std::fs::File, serde_yaml::value::Value>(file) {
                if let Some(palette) = yaml["palette"].as_str() {
                    match palette {
                        "red-blue" => {colorer = Colorer::color_red_blue as fn(usize) -> Color;},
                        "red-green" => {colorer = Colorer::color_red_green as fn(usize) -> Color;},
                        _ => ()
                    }
                }
            };
        };
        colorer
    };
}

/// Returns a color based on the extension.
/// Those colors will always be the same, but a palette is defined from a yaml value.
pub fn extension_color(extension: &str) -> Color {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(extension.as_bytes());
    COLORER(hasher.finish() as usize)
}
