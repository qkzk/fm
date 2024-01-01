use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::modes::{FileInfo, Preview, Users};

pub struct Previewer {
    tx: mpsc::Sender<Option<PathBuf>>,
    handle: thread::JoinHandle<Result<()>>,
}

impl Previewer {
    pub fn new(preview_cache: Arc<Mutex<PreviewCache>>) -> Self {
        let (tx, rx) = mpsc::channel::<Option<PathBuf>>();

        let handle = thread::spawn(move || -> Result<()> {
            loop {
                match rx.try_recv() {
                    Ok(Some(path)) => match preview_cache.lock() {
                        Ok(mut preview_cache) => {
                            preview_cache.update(&path)?;
                            drop(preview_cache);
                        }
                        Err(error) => return Err(anyhow!("Error locker preview_cache: {error}")),
                    },
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        crate::log_info!("terminating previewer");
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }
            Ok(())
        });
        Self { tx, handle }
    }
}

#[derive(Default)]
pub struct PreviewCache {
    cache: HashMap<PathBuf, Preview>,
    paths: Vec<PathBuf>,
}

impl PreviewCache {
    const SIZE_LIMIT: usize = 100;

    /// Returns an optional reference to the preview.
    pub fn read(&self, path: &Path) -> Option<&Preview> {
        self.cache.get(path)
    }

    /// True iff the cache aleady contains a preview of path.
    ///
    /// # Errors
    ///
    /// May fail if:
    /// - FileInfo fail,
    /// - Preview fail.
    pub fn update(&mut self, path: &Path) -> Result<bool> {
        if self.cache.contains_key(path) {
            return Ok(false);
        };
        self.add_preview(path)?;
        self.limit_size();
        Ok(true)
    }

    fn add_preview(&mut self, path: &Path) -> Result<()> {
        self.cache
            .insert(path.to_path_buf(), Self::make_preview(path)?);
        self.paths.push(path.to_path_buf());
        Ok(())
    }

    fn make_preview(path: &Path) -> Result<Preview> {
        Preview::file(&FileInfo::from_path_with_name(
            path,
            &path.file_name().context("")?.to_string_lossy(),
            &Users::default(),
        )?)
    }

    fn limit_size(&mut self) {
        if self.cache.len() > Self::SIZE_LIMIT {
            let path = self.paths.remove(0);
            self.cache.remove(&path);
        }
    }
}
