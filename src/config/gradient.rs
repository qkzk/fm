use tuikit::attr::Color;

use crate::config::COLORER;
use crate::config::GRADIENT_NORMAL_FILE;

/// No attr but a few static methods.
pub struct NormalFileColorer {}

impl NormalFileColorer {
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

    /// Picks a blueish/greenish color on color picker hexagon's perimeter.
    pub fn color_blue_green(hash: usize) -> Color {
        (128..255)
            .map(|g| Color::Rgb(0, g, 255))
            .chain((128..255).map(|b| Color::Rgb(0, 255, b)))
            .nth(hash % 254)
            .unwrap()
    }

    /// Picks a redish/blueish color on color picker hexagon's perimeter.
    pub fn color_blue_red(hash: usize) -> Color {
        (128..255)
            .map(|r| Color::Rgb(r, 0, 255))
            .chain((128..255).map(|b| Color::Rgb(255, 0, b)))
            .nth(hash % 254)
            .unwrap()
    }

    /// Picks a redish/greenish color on color picker hexagon's perimeter.
    pub fn color_green_red(hash: usize) -> Color {
        (128..255)
            .map(|g| Color::Rgb(255, g, 0))
            .chain((128..255).map(|r| Color::Rgb(r, 255, 0)))
            .nth(hash % 254)
            .unwrap()
    }

    pub fn color_custom(hash: usize) -> Color {
        let gradient = GRADIENT_NORMAL_FILE
            .get()
            .expect("Gradient normal file should be set");
        gradient.step(hash % 254).as_tuikit()
    }
}

fn sum_hash(string: &str) -> usize {
    let hash: usize = string
        .as_bytes()
        .iter()
        .map(|s| *s as usize)
        .reduce(|acc, elt| acc.saturating_mul(254).saturating_add(elt))
        .unwrap_or_default();
    hash & 254
}

/// Returns a color based on the extension.
/// Those colors will always be the same, but a palette is defined from a yaml value.
pub fn extension_color(extension: &str) -> Color {
    COLORER.get().expect("Colorer should be set")(sum_hash(extension))
}

#[derive(Debug, Clone, Copy)]
pub struct ColorG {
    r: u8,
    g: u8,
    b: u8,
}

impl Default for ColorG {
    fn default() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 0,
        }
    }
}

impl ColorG {
    pub fn new(triplet: (u8, u8, u8)) -> Self {
        Self {
            r: triplet.0,
            g: triplet.1,
            b: triplet.2,
        }
    }
    /// Parse a tuikit color into it's rgb values.
    /// Non parsable colors returns None.
    pub fn from_tuikit(color: Color) -> Option<Self> {
        match color {
            Color::Rgb(r, g, b) => Some(Self { r, g, b }),
            _ => None,
        }
    }

    fn as_tuikit(&self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Gradient {
    start: ColorG,
    end: ColorG,
    step_ratio: f32,
    len: usize,
}

impl Gradient {
    pub fn new(start: ColorG, end: ColorG, len: usize) -> Self {
        let step_ratio = 1_f32 / len as f32;
        Self {
            start,
            end,
            step_ratio,
            len,
        }
    }

    fn step(&self, step: usize) -> ColorG {
        let position = self.step_ratio * step as f32;

        let r = self.start.r as f32 + (self.end.r as f32 - self.start.r as f32) * position;
        let g = self.start.g as f32 + (self.end.g as f32 - self.start.g as f32) * position;
        let b = self.start.b as f32 + (self.end.b as f32 - self.start.b as f32) * position;

        ColorG {
            r: r.round() as u8,
            g: g.round() as u8,
            b: b.round() as u8,
        }
    }

    pub fn gradient(&self) -> impl Iterator<Item = Color> + '_ {
        (0..self.len).map(|step| self.step(step).as_tuikit())
    }
}
