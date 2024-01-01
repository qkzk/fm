use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::modes::{FileInfo, Preview, Users};

pub struct Previewer {
    pub tx: mpsc::Sender<Option<PathBuf>>,
    pub handle_receiver: thread::JoinHandle<Result<()>>,
    pub handle_builder: thread::JoinHandle<Result<()>>,
}

impl Previewer {
    pub fn new(preview_cache: Arc<Mutex<PreviewCache>>) -> Self {
        let (tx, rx) = mpsc::channel::<Option<PathBuf>>();
        let queue: Arc<Mutex<VecDeque<PathBuf>>> = Arc::new(Mutex::new(VecDeque::new()));
        let queue2 = queue.clone();

        let handle_receiver = thread::spawn(move || -> Result<()> {
            loop {
                match rx.try_recv() {
                    Ok(Some(path)) => match queue2.lock() {
                        Ok(mut queue) => {
                            queue.push_back(path);
                            drop(queue);
                        }
                        Err(error) => return Err(anyhow!("Error locking queue: {error}")),
                    },
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        crate::log_info!("terminating previewer");
                        break;
                    }
                    Err(TryRecvError::Empty) => {
                        thread::sleep(Duration::from_millis(33));
                    }
                }
            }
            Ok(())
        });

        let handle_builder = thread::spawn(move || loop {
            match queue.lock() {
                Err(error) => return Err(anyhow!("error locking queue lock: {error}")),
                Ok(mut queue) => {
                    match queue.pop_front() {
                        Some(path) => match preview_cache.lock() {
                            Ok(mut preview_cache) => {
                                preview_cache.update(&path)?;
                                drop(preview_cache);
                            }
                            Err(error) => {
                                return Err(anyhow!("Error locking preview_cache: {error}"))
                            }
                        },
                        None => {
                            thread::sleep(Duration::from_millis(33));
                        }
                    };
                    drop(queue);
                }
            }
        });
        Self {
            tx,
            handle_receiver,
            handle_builder,
        }
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
            crate::log_info!("key {path} already in cache", path = path.display());
            return Ok(false);
        };
        self.add_preview(path)?;
        self.limit_size();
        Ok(true)
    }

    pub fn add_made_preview(&mut self, path: &Path, preview: Preview) {
        self.cache.insert(path.to_path_buf(), preview);
        self.paths.push(path.to_path_buf());
        self.limit_size();
    }

    fn add_preview(&mut self, path: &Path) -> Result<()> {
        self.cache
            .insert(path.to_path_buf(), Self::make_preview(path)?);
        self.paths.push(path.to_path_buf());
        crate::log_info!("added {path} to cache", path = path.display());
        Ok(())
    }

    fn make_preview(path: &Path) -> Result<Preview> {
        Preview::new(&FileInfo::from_path_with_name(
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
