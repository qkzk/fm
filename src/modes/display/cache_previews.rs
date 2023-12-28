use std::path::Path;
use std::sync::Arc;
use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use tokio::sync::{mpsc, Mutex};

use crate::modes::Preview;

pub async fn update_cache(
    mut rx: mpsc::Receiver<PathBuf>,
    cache_previews: Arc<Mutex<CachePreviews>>,
) -> Result<()> {
    while let Some(path) = rx.recv().await {
        let mut cache = cache_previews.lock().await;
        cache.update(&path)?;
    }
    Ok(())
}

#[derive(Default)]
pub struct CachePreviews {
    previews: HashMap<PathBuf, Preview>,
    paths: Vec<PathBuf>,
}

impl CachePreviews {
    const PREVIEWS_CAPACITY: usize = 100;

    pub fn contains(&self, path: &Path) -> bool {
        let res = self.previews.contains_key(path);
        let log_string = if res { "hit" } else { "miss" };
        crate::log_info!("cache_preview: {log_string} {path}", path = path.display());
        res
    }

    pub fn read(&self, path: &Path) -> Option<&Preview> {
        self.previews.get(path)
    }

    /// Insert a new preview in the cache.
    /// Returns `True` if the preview was inserted, false otherwise.
    ///
    /// # Errors
    ///
    /// May fail if the preview can't be created.
    /// See [`crate::modes::Preview`] for more information.
    pub fn update(&mut self, path: &Path) -> Result<bool> {
        if self.previews.get(path).is_some() {
            return Ok(false);
        }
        if self.paths.len() >= Self::PREVIEWS_CAPACITY {
            self.remove_last();
        }
        self.paths.insert(0, path.to_owned());
        let preview = Preview::file(path)?;
        self.previews.insert(path.to_owned(), preview);

        Ok(true)
    }

    fn remove_last(&mut self) {
        let Some(last_path) = self.paths.pop() else {
            return;
        };
        self.previews.remove(&last_path);
    }
}
