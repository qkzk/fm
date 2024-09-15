use tuikit::attr::Color;

use crate::config::ARRAY_GRADIENT;
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
    pub r: u8,
    pub g: u8,
    pub b: u8,
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

    pub fn as_tuikit(&self) -> Color {
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

/// Convert a string color into a `tuikit::Color` instance.
pub fn str_to_tuikit<S>(color: S) -> Color
where
    S: AsRef<str>,
{
    match color.as_ref() {
        "white" => Color::WHITE,
        "red" => Color::RED,
        "green" => Color::GREEN,
        "blue" => Color::BLUE,
        "yellow" => Color::YELLOW,
        "cyan" => Color::CYAN,
        "magenta" => Color::MAGENTA,
        "black" => Color::BLACK,
        "light_white" => Color::LIGHT_WHITE,
        "light_red" => Color::LIGHT_RED,
        "light_green" => Color::LIGHT_GREEN,
        "light_blue" => Color::LIGHT_BLUE,
        "light_yellow" => Color::LIGHT_YELLOW,
        "light_cyan" => Color::LIGHT_CYAN,
        "light_magenta" => Color::LIGHT_MAGENTA,
        "light_black" => Color::LIGHT_BLACK,
        color => parse_rgb_color(color),
    }
}

/// Tries to parse an unknown color into a `Color::Rgb(u8, u8, u8)`
/// rgb and hexadecimal formats should never fail.
/// Other formats are unknown.
/// rgb( 123,   78,          0) -> Color::Rgb(123, 78, 0)
/// #FF00FF -> Color::Rgb(255, 0, 255)
/// Unreadable colors are replaced by `Color::default()` which is white.
fn parse_rgb_color(color: &str) -> Color {
    if let Some(triplet) = parse_text_triplet(color) {
        return Color::Rgb(triplet.0, triplet.1, triplet.2);
    }
    Color::default()
}

pub fn parse_text_triplet(color: &str) -> Option<(u8, u8, u8)> {
    let color = color.to_lowercase();
    if color.starts_with("rgb(") && color.ends_with(')') {
        return parse_rgb_triplet(&color);
    } else if color.starts_with('#') && color.len() >= 7 {
        return parse_hex_triplet(&color);
    }
    None
}

fn parse_rgb_triplet(color: &str) -> Option<(u8, u8, u8)> {
    let triplet: Vec<u8> = color
        .replace("rgb(", "")
        .replace([')', ' '], "")
        .trim()
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect();
    if triplet.len() == 3 {
        return Some((triplet[0], triplet[1], triplet[2]));
    }
    None
}

fn parse_hex_triplet(color: &str) -> Option<(u8, u8, u8)> {
    let r = parse_hex_byte(&color[1..3])?;
    let g = parse_hex_byte(&color[3..5])?;
    let b = parse_hex_byte(&color[5..7])?;
    Some((r, g, b))
}

fn parse_hex_byte(byte: &str) -> Option<u8> {
    u8::from_str_radix(byte, 16).ok()
}
