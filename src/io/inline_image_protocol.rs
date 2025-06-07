use std::fs::File;
use std::io::{stdout, Read, Write};

use anyhow::Result;
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::layout::Rect;

use crate::io::ImageDisplayer;
use crate::log_info;
use crate::modes::DisplayedImage;

#[derive(Default, Debug)]
pub struct InlineImage {
    last_displayed: Option<(String, Rect)>,
    is_displaying: bool,
}

impl ImageDisplayer for InlineImage {
    fn draw(&mut self, image: &DisplayedImage, rect: Rect) -> Result<()> {
        let path = &image.images[image.image_index()].to_string_lossy();
        let bytes = Self::read_as_bytes(path)?;
        let encoded = iterm2img::from_bytes(bytes)
            .width(rect.width as u64)
            .height(rect.height as u64)
            .preserve_aspect_ratio(true)
            .inline(true)
            .build();
        log_info!("inline image: draws {path} {rect}");
        disable_raw_mode()?;
        execute!(stdout(), SavePosition, MoveTo(rect.x, rect.y))?;
        stdout().write_all(encoded.as_bytes())?;
        execute!(stdout(), RestorePosition)?;
        enable_raw_mode()?;
        self.is_displaying = true;
        self.last_displayed = Some((path.to_string(), rect));
        Ok(())
    }

    fn clear(&mut self, _: &DisplayedImage) -> Result<()> {
        log_info!("inline image clear {last:?}", last = self.last_displayed);
        if let Some((_, rect)) = self.last_displayed {
            let s = " ".repeat(rect.width as usize);
            disable_raw_mode()?;
            execute!(stdout(), SavePosition, MoveTo(rect.x, rect.y))?;
            for y in rect.top()..rect.bottom() {
                execute!(stdout(), MoveTo(rect.x, y))?;
                stdout().write_all(s.as_bytes())?;
            }
            execute!(stdout(), RestorePosition)?;
            enable_raw_mode()?;
        }
        self.last_displayed = None;
        self.is_displaying = false;
        Ok(())
    }

    fn clear_all(&mut self) -> Result<()> {
        log_info!(
            "inline image clear ALL - last {last:?}",
            last = self.last_displayed
        );
        if let Some((_, rect)) = self.last_displayed {
            let s = " ".repeat(rect.width as usize);
            disable_raw_mode()?;
            execute!(stdout(), SavePosition, MoveTo(rect.x, rect.y))?;
            for y in rect.top()..rect.bottom() {
                execute!(stdout(), MoveTo(rect.x, y))?;
                stdout().write_all(s.as_bytes())?;
            }
            execute!(stdout(), RestorePosition)?;
            enable_raw_mode()?;
            log_info!("inline image done clearing all");
        }
        self.last_displayed = None;
        self.is_displaying = false;
        Ok(())
    }
}

impl InlineImage {
    fn read_as_bytes<P>(path: P) -> Result<Vec<u8>>
    where
        P: AsRef<str>,
    {
        let mut f = File::open(path.as_ref())?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        Ok(buf)
    }
}
