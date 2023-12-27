use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;

use crate::modes::FileInfo;
use crate::modes::Preview;

pub struct CachePreviews {
    previews: HashMap<PathBuf, Preview>,
    paths: Vec<PathBuf>,
}

impl Default for CachePreviews {
    fn default() -> Self {
        Self {
            previews: HashMap::new(),
            paths: vec![],
        }
    }
}

impl CachePreviews {
    const PREVIEWS_CAPACITY: usize = 5;

    pub fn contains(&self, path: &Path) -> bool {
        self.previews.contains_key(path)
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
    pub fn update(&mut self, file: &FileInfo) -> Result<bool> {
        let path = &file.path.to_path_buf();
        if self.previews.get(path).is_some() {
            return Ok(false);
        }
        if self.paths.len() >= Self::PREVIEWS_CAPACITY {
            self.remove_last();
        }
        self.paths.insert(0, path.to_owned());
        let preview = Preview::file(file)?;
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
