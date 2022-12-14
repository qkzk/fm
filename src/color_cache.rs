use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hasher;

use tuikit::attr::Color;

/// Holds a map of extension name to color.
/// Every extension is associated to a color which is only computed once
/// per run. This trades a bit of memory for a bit of CPU.
#[derive(Default, Clone, Debug)]
pub struct ColorCache {
    cache: RefCell<HashMap<String, Color>>,
}

impl ColorCache {
    /// Returns a color for any possible extension.
    /// The color is cached within the struct, avoiding multiple calculations.
    pub fn extension_color(&self, extension: &str) -> Color {
        let mut cache = self.cache.borrow_mut();
        if let Some(color) = cache.get(extension) {
            color.to_owned()
        } else {
            let color = extension_color(extension);
            cache.insert(extension.to_owned(), color);
            color
        }
    }
}

/// Picks a blueish/greenish color on color picker hexagon's perimeter.
fn color(coords: usize) -> Color {
    (128..255)
        .map(|b| Color::Rgb(0, 255, b))
        .chain((128..255).map(|g| Color::Rgb(0, g, 255)))
        .chain((128..255).rev().map(|b| Color::Rgb(0, 255, b)))
        .chain((128..255).rev().map(|g| Color::Rgb(0, g, 255)))
        .nth(coords % 508)
        .unwrap()
}

fn extension_color(extension: &str) -> Color {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(extension.as_bytes());
    color(hasher.finish() as usize)
}
