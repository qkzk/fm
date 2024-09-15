use anyhow::{anyhow, Result};
use tuikit::attr::Color;

use crate::config::configurable_static::ARRAY_GRADIENT;
use crate::config::parse_text_triplet;
use crate::config::COLORER;

/// No attr but a few static methods.
pub struct NormalFileColorer {}

impl NormalFileColorer {
    pub fn colorer(hash: usize) -> Color {
        let gradient = ARRAY_GRADIENT
            .get()
            .expect("Gradient normal file should be set");
        gradient[hash % 254]
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

    fn from_ansi_desc(color_name: &str) -> Option<Self> {
        match color_name.to_lowercase().as_str() {
            "black" => Some(Self::new((0, 0, 0))),
            "red" => Some(Self::new((255, 0, 0))),
            "green" => Some(Self::new((0, 255, 0))),
            "yellow" => Some(Self::new((255, 255, 0))),
            "blue" => Some(Self::new((0, 0, 255))),
            "magenta" => Some(Self::new((255, 0, 255))),
            "cyan" => Some(Self::new((0, 255, 255))),
            "white" => Some(Self::new((255, 255, 255))),

            "light_black" | "bright_black" => Some(Self::new((85, 85, 85))),
            "light_red" | "bright_red" => Some(Self::new((255, 85, 85))),
            "light_green" | "bright_green" => Some(Self::new((85, 255, 85))),
            "light_yellow" | "bright_yellow" => Some(Self::new((255, 255, 85))),
            "light_blue" | "bright_blue" => Some(Self::new((85, 85, 255))),
            "light_magenta" | "bright_magenta" => Some(Self::new((255, 85, 255))),
            "light_cyan" | "bright_cyan" => Some(Self::new((85, 255, 255))),
            "light_white" | "bright_white" => Some(Self::new((255, 255, 255))),

            _ => None,
        }
    }

    /// Parse a color in any kind of format: ANSI text (red etc.), rgb or hex.
    /// The parser for ANSI text colors recognize all common name whatever the capitalization.
    /// It doesn't try to parse rgb or hex values
    /// Only the default values are used. If the user changed "red" to be #ffff00 (which is yellow...)
    /// in its terminal setup, we can't know. So, what the user will get on screen is red: #ff0000.
    pub fn parse_any_color(text: &str) -> Option<Self> {
        if let Some(triplet) = parse_text_triplet(text) {
            Some(Self::new(triplet))
        } else {
            Self::from_ansi_desc(text)
        }
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

    pub fn into_array(&self) -> Result<[Color; 254]> {
        let v: Vec<Color> = self.gradient().collect();
        let a = v.try_into().map_err(|e| anyhow!("Couldn't dump {e:?}"))?;
        Ok(a)
    }

    pub fn gradient(&self) -> impl Iterator<Item = Color> + '_ {
        (0..self.len).map(|step| self.step(step).as_tuikit())
    }
}
