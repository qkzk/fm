use anyhow::{anyhow, Result};
use tuikit::attr::Color;

use crate::config::ColorG;
use crate::config::MAX_GRADIENT_NORMAL;

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

    pub fn as_array(&self) -> Result<[Color; MAX_GRADIENT_NORMAL]> {
        self.gradient()
            .collect::<Vec<Color>>()
            .try_into()
            .map_err(|_| anyhow!("Couldn't create an array for gradient"))
    }

    pub fn gradient(&self) -> impl Iterator<Item = Color> + '_ {
        (0..self.len).map(|step| self.step(step).as_tuikit())
    }
}