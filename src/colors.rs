use std::hash::Hasher;

use tuikit::attr::Color;

/// Picks a blueish/greenish color on color picker hexagon's perimeter.
fn color(hash: usize) -> Color {
    (128..255)
        .map(|b| Color::Rgb(0, 255, b))
        .chain((128..255).map(|g| Color::Rgb(0, g, 255)))
        .nth(hash % 254)
        .unwrap()
}

pub fn extension_color(extension: &str) -> Color {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(extension.as_bytes());
    color(hasher.finish() as usize)
}
