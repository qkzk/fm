use ratatui::style::Color;

use crate::config::{ARRAY_GRADIENT, COLORER};

pub const MAX_GRADIENT_NORMAL: usize = 254;

/// No style but a method to color give a color for any extension.
/// Extension should be hashed to an usize first.
pub struct NormalFileColorer {}

impl NormalFileColorer {
    #[inline]
    pub fn colorer(hash: usize) -> Color {
        let gradient = ARRAY_GRADIENT
            .get()
            .expect("Gradient normal file should be set");
        gradient[hash % MAX_GRADIENT_NORMAL]
    }
}

#[inline]
fn sum_hash(string: &str) -> usize {
    let hash: usize = string
        .as_bytes()
        .iter()
        .map(|s| *s as usize)
        .reduce(|acc, elt| acc.saturating_mul(MAX_GRADIENT_NORMAL).saturating_add(elt))
        .unwrap_or_default();
    hash & MAX_GRADIENT_NORMAL
}

/// Returns a color based on the extension.
/// Those colors will always be the same, but a palette is defined from a yaml value.
#[inline]
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
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    /// Parse a tuikit color into it's rgb values.
    /// Non parsable colors returns None.
    pub fn from_ratatui(color: Color) -> Option<Self> {
        match color {
            Color::Rgb(r, g, b) => Some(Self { r, g, b }),
            _ => None,
        }
    }

    pub fn as_ratatui(&self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }

    #[rustfmt::skip]
    fn from_ansi_desc(color_name: &str) -> Option<Self> {
        match color_name.to_lowercase().as_str() {
            "black"         => Some(Self::new(0,     0,   0)),
            "red"           => Some(Self::new(255,   0,   0)),
            "green"         => Some(Self::new(0,   255,   0)),
            "yellow"        => Some(Self::new(255, 255,   0)),
            "blue"          => Some(Self::new(0,     0, 255)),
            "magenta"       => Some(Self::new(255,   0, 255)),
            "cyan"          => Some(Self::new(0,   255, 255)),
            "white"         => Some(Self::new(255, 255, 255)),

            "light_black"   => Some(Self::new(85,   85,  85)),
            "light_red"     => Some(Self::new(255,  85,  85)),
            "light_green"   => Some(Self::new(85,  255,  85)),
            "light_yellow"  => Some(Self::new(255, 255,  85)),
            "light_blue"    => Some(Self::new(85,   85, 255)),
            "light_magenta" => Some(Self::new(255,  85, 255)),
            "light_cyan"    => Some(Self::new(85,  255, 255)),
            "light_white"   => Some(Self::new(255, 255, 255)),

            _               => None,
        }
    }

    /// Parse a color in any kind of format: ANSI text (red etc.), rgb or hex.
    /// The parser for ANSI text colors recognize all common name whatever the capitalization.
    /// It doesn't try to parse rgb or hex values
    /// Only the default values are used. If the user changed "red" to be #ffff00 (which is yellow...)
    /// in its terminal setup, we can't know. So, what the user will get on screen is red: #ff0000.
    pub fn parse_any_color(text: &str) -> Option<Self> {
        match parse_text_triplet(text) {
            Some((r, g, b)) => Some(Self::new(r, g, b)),
            None => Self::from_ansi_desc(text),
        }
    }
}

/// Tries to parse a string color into a [`tuikit::attr::Color`].
/// Ansi colors are converted to their corresponding version in tuikit.
/// rgb and hexadecimal formats are parsed also.
/// rgb( 123,   78,          0)     -> Color::Rgb(123, 78, 0)
/// #FF00FF                         -> Color::Rgb(255, 0, 255)
/// Other formats are unknown.
/// Unreadable colors are replaced by `Color::default()` which is white.
#[rustfmt::skip]
pub fn str_to_ratatui<S>(color: S) -> Color
where
    S: AsRef<str>,
{
    match color.as_ref() {
        "white"         => Color::White,
        "red"           => Color::Red,
        "green"         => Color::Green,
        "blue"          => Color::Blue,
        "yellow"        => Color::Yellow,
        "cyan"          => Color::Cyan,
        "magenta"       => Color::Magenta,
        "black"         => Color::Black,
        // TODO! light white
        "light_white"   => Color::White,
        "light_red"     => Color::LightRed,
        "light_green"   => Color::LightGreen,
        "light_blue"    => Color::LightBlue,
        "light_yellow"  => Color::LightYellow,
        "light_cyan"    => Color::LightCyan,
        "light_magenta" => Color::LightMagenta,
        // TODO! light black
        "light_black"   => Color::Black,
        color     => parse_text_triplet_unfaillible(color),
    }
}

fn parse_text_triplet_unfaillible(color: &str) -> Color {
    match parse_text_triplet(color) {
        Some((r, g, b)) => Color::Rgb(r, g, b),
        None => Color::Rgb(0, 0, 0),
    }
}

fn parse_text_triplet(color: &str) -> Option<(u8, u8, u8)> {
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
