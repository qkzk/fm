use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hasher;

use tuikit::attr::Color;

pub struct ColorCache {
    cache: RefCell<HashMap<String, Color>>,
}

impl Default for ColorCache {
    fn default() -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
        }
    }
}

impl ColorCache {
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

/// Picks a color on color picker hexagon's perimeter.
fn color(coords: usize) -> Color {
    (0..255)
        .map(|g| Color::Rgb(255, g, 0))
        .chain((0..255).rev().map(|r| Color::Rgb(r, 255, 0)))
        .chain((0..255).map(|b| Color::Rgb(0, 255, b)))
        .chain((0..255).rev().map(|g| Color::Rgb(0, g, 255)))
        .chain((0..255).map(|r| Color::Rgb(r, 0, 255)))
        .chain((0..255).rev().map(|b| Color::Rgb(255, 0, b)))
        .nth(coords % 1536)
        .unwrap()
}

fn extension_color(extension: &str) -> Color {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(extension.as_bytes());
    color(hasher.finish() as usize)
}