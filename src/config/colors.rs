use std::hash::Hasher;

use tuikit::attr::Color;

use crate::config::{COLORER, START_COLOR, STOP_COLOR};

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
        let lerp = lerp_color(*START_COLOR, *STOP_COLOR, (hash % 255) as u8);
        Color::Rgb(lerp.0, lerp.1, lerp.2)
    }
}

/// Returns a color based on the extension.
/// Those colors will always be the same, but a palette is defined from a yaml value.
pub fn extension_color(extension: &str) -> Color {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(extension.as_bytes());
    COLORER(hasher.finish() as usize)
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

    fn gradient_step(&self, step: usize) -> ColorG {
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
        (0..self.len).map(move |step| self.gradient_step(step).as_tuikit())
    }
}

fn lerp_color(start: (u8, u8, u8), end: (u8, u8, u8), step: u8) -> (u8, u8, u8) {
    let step = step as f32 / 255.0;
    let (r1, g1, b1) = (start.0 as f32, start.1 as f32, start.2 as f32);
    let (r2, g2, b2) = (end.0 as f32, end.1 as f32, end.2 as f32);
    (
        (r1 + (r2 - r1) * step).round() as u8,
        (g1 + (g2 - g1) * step).round() as u8,
        (b1 + (b2 - b1) * step).round() as u8,
    )
}
