use std::hash::Hasher;

use tuikit::attr::Color;

use crate::config::COLORER;

/// No attr but 3 static methods.
pub struct Colorer {}

impl Colorer {
    /// Picks a blueish/greenish color on color picker hexagon's perimeter.
    pub fn color_green_blue(hash: usize) -> Color {
        (128..255)
            .map(|b| Color::Rgb(0, 255, b))
            .chain((128..255).map(|g| Color::Rgb(0, g, 255)))
            .nth(hash % 254)
            .unwrap()
    }

    /// Picks a redish/blueish color on color picker hexagon's perimeter.
    pub fn color_red_blue(hash: usize) -> Color {
        (128..255)
            .map(|b| Color::Rgb(255, 0, b))
            .chain((128..255).map(|r| Color::Rgb(r, 0, 255)))
            .nth(hash % 254)
            .unwrap()
    }

    /// Picks a redish/greenish color on color picker hexagon's perimeter.
    pub fn color_red_green(hash: usize) -> Color {
        (128..255)
            .map(|r| Color::Rgb(r, 255, 0))
            .chain((128..255).map(|g| Color::Rgb(255, g, 0)))
            .nth(hash % 254)
            .unwrap()
    }
}

/// Returns a color based on the extension.
/// Those colors will always be the same, but a palette is defined from a yaml value.
pub fn extension_color(extension: &str) -> Color {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(extension.as_bytes());
    COLORER(hasher.finish() as usize)
}
