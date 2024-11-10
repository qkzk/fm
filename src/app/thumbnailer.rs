use std::{
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use crate::modes::Thumbnail;
use crate::{common::TMP_THUMBNAILS_DIR, log_info};

fn make_thumbnail(path: PathBuf) {
    match Thumbnail::create_video(&path.to_string_lossy()) {
        Ok(_) => log_info!("thumbnail built successfully"),
        Err(e) => log_info!("error building thumbnail {e}"),
    }
}

/// Video thumbnail builder.
///
/// Store videos paths to be thumbnailed in a thread safe vector.
/// Keep track of a bunch of workers.
///
/// The first available workers dequeue a path and build its thumbnails.
///
/// A bunch of video paths can be added to the collection.
/// They should all be _videos_ which can be thumbnailed.
#[derive(Debug)]
pub struct ThumbnailManager {
    queue: Arc<Mutex<Vec<PathBuf>>>,
    workers: Vec<Worker>,
}

impl Default for ThumbnailManager {
    fn default() -> Self {
        Self::create_thumbnail_dir_if_not_exist();
        let num_workers = Self::default_thread_count();
        let mut workers = Vec::with_capacity(num_workers);
        let queue = Arc::new(Mutex::new(vec![]));
        for id in 0..num_workers {
            workers.push(Worker::new(id, queue.clone()));
        }

        Self { queue, workers }
    }
}

impl ThumbnailManager {
    fn create_thumbnail_dir_if_not_exist() {
        if Path::new(TMP_THUMBNAILS_DIR).exists() {
            return;
        }
        if let Err(error) = create_dir_all(TMP_THUMBNAILS_DIR) {
            log_info!("Coudln't create {TMP_THUMBNAILS_DIR}. Error: {error}");
        }
    }

    fn default_thread_count() -> usize {
        thread::available_parallelism()
            .map(|it| it.get().checked_sub(6).unwrap_or(1))
            .unwrap_or(1)
    }

    /// Add all received files to the queue.
    ///
    /// They will be dealt with by the first available worker.
    pub fn enqueue(&self, mut videos: Vec<PathBuf>) {
        let Ok(mut locked_queue) = self.queue.lock() else {
            log_info!("ThumbnailManager couldn't lock the queue");
            return;
        };
        locked_queue.append(&mut videos);
        drop(locked_queue);
    }

    /// Clear the queue.
    ///
    /// Remove all videos awaiting to be thumbnailed from the queue.
    pub fn clear(&self) {
        let Ok(mut locked_queue) = self.queue.lock() else {
            log_info!("ThumbnailManager couldn't lock the queue");
            return;
        };
        locked_queue.clear();
        drop(locked_queue);
    }

    /// Quit.
    ///
    /// Stop all the running workers.
    /// Send a message to their mpsc::channel telling them to break their running loop.
    pub fn quit(&self) {
        for worker in &self.workers {
            worker.quit()
        }
        log_info!("ThumbnailManager quit");
    }
}

#[derive(Debug)]
pub struct Worker {
    id: usize,
    tx: mpsc::Sender<()>,
}

impl Worker {
    fn new(id: usize, queue: Arc<Mutex<Vec<PathBuf>>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let _ = thread::spawn(move || Self::runner(id, queue, rx));
        Self { id, tx }
    }

    fn quit(&self) {
        let _ = self.tx.send(());
        log_info!("Worker {id} quit", id = self.id);
    }

    fn runner(id: usize, queue: Arc<Mutex<Vec<PathBuf>>>, rx: mpsc::Receiver<()>) {
        loop {
            let Ok(mut locked_queue) = queue.lock() else {
                log_info!("Worker {id} couldn't lock the queue");
                continue;
            };

            if !locked_queue.is_empty() {
                let task = locked_queue.remove(0);
                log_info!("Worker {id} received task {p}", p = task.display());
                make_thumbnail(task);
            }
            drop(locked_queue);
            if let Ok(()) = rx.try_recv() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
