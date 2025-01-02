use std::{
    collections::VecDeque,
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use parking_lot::Mutex;

use crate::modes::Thumbnail;
use crate::{common::TMP_THUMBNAILS_DIR, log_info};

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
    queue: Arc<Mutex<VecDeque<PathBuf>>>,
    is_empty: Arc<AtomicBool>,
    _workers: Vec<Worker>,
}

impl Default for ThumbnailManager {
    fn default() -> Self {
        Self::create_thumbnail_dir_if_not_exist();
        let num_workers = Self::default_thread_count();
        let mut _workers = Vec::with_capacity(num_workers);
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let is_empty = Arc::new(AtomicBool::new(true));
        for id in 0..num_workers {
            _workers.push(Worker::new(id, queue.clone(), is_empty.clone()));
        }

        Self {
            queue,
            _workers,
            is_empty,
        }
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
    pub fn enqueue(&self, mut videos: VecDeque<PathBuf>) {
        let mut locked_queue = self.queue.lock();
        // TODO remove when satisfied
        log_info!(
            "Enqueuing {len} videos to thumbnail queue",
            len = videos.len()
        );
        locked_queue.append(&mut videos);
        drop(locked_queue);
        self.is_empty.store(false, Ordering::SeqCst);
    }

    /// Clear the queue.
    ///
    /// Remove all videos awaiting to be thumbnailed from the queue.
    pub fn clear(&self) {
        let mut locked_queue = self.queue.lock();
        locked_queue.clear();
        drop(locked_queue);
    }
}

#[derive(Debug)]
pub struct Worker {
    _handle: JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, queue: Arc<Mutex<VecDeque<PathBuf>>>, is_empty: Arc<AtomicBool>) -> Self {
        let _handle = thread::spawn(move || Self::runner(id, queue, is_empty));
        Self { _handle }
    }

    fn runner(id: usize, queue: Arc<Mutex<VecDeque<PathBuf>>>, is_empty: Arc<AtomicBool>) {
        loop {
            Self::advance_queue(id, &queue, &is_empty);
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn advance_queue(id: usize, queue: &Arc<Mutex<VecDeque<PathBuf>>>, is_empty: &Arc<AtomicBool>) {
        if is_empty.load(Ordering::SeqCst) {
            return;
        }
        let mut locked_queue = queue.lock();
        let Some(path) = locked_queue.pop_front() else {
            return;
        };
        if locked_queue.is_empty() {
            is_empty.store(true, Ordering::SeqCst);
        }
        drop(locked_queue);
        log_info!("Worker {id} received task {p}", p = path.display());
        Self::make_thumbnail(path);
    }

    fn make_thumbnail(path: PathBuf) {
        match Thumbnail::create_video(&path.to_string_lossy()) {
            Ok(_) => log_info!("thumbnail built successfully"),
            Err(e) => log_info!("error building thumbnail {e}"),
        }
    }
}
