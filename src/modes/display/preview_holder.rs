use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;

use parking_lot::{Mutex, RwLock};

use crate::log_info;
use crate::modes::Preview;
use crate::modes::Ueberzug;
use crate::modes::Users;

/// Holds thre previews and a threadpool to create them.
/// Allow creation of preview for a single file or a collection.
#[derive(Clone)]
pub struct PreviewHolder {
    pub previews: Arc<RwLock<BTreeMap<PathBuf, Arc<Preview>>>>,
    pool: ThreadPool,
    users: Users,
    previewed_left: Option<PathBuf>,
    previewed_right: Option<PathBuf>,
}

impl Default for PreviewHolder {
    fn default() -> Self {
        let users = Users::new();
        let previews = Arc::new(RwLock::new(BTreeMap::new()));
        let pool = ThreadPool::new(Self::NB_WORKERS);
        let previewed_left = None;
        let previewed_right = None;
        Self {
            previews,
            pool,
            users,
            previewed_left,
            previewed_right,
        }
    }
}

impl PreviewHolder {
    /// Maximum number of previews in the collection
    const MAX_PREVIEWS: usize = 500;

    // TODO: optionable for low config ?
    /// Number of preview creation threads.
    const NB_WORKERS: usize = 4;

    /// Returns a preview from its path. None if it's not present.
    pub fn get(&self, p: &Path) -> Option<Arc<Preview>> {
        self.previews.read().get(p).cloned()
    }

    /// Deletes all previews from the collection
    fn clear(&mut self) {
        self.previews.write().clear();
    }

    /// Add an already created preview to the collection.
    /// It's used for "help", "log" and "command" where we don't preview a file but a custom text.
    pub fn put_preview<P>(&mut self, path: P, preview: Preview)
    where
        P: AsRef<Path>,
    {
        self.previews
            .write()
            .insert(path.as_ref().to_owned(), Arc::new(preview));
    }

    pub fn set_previewed(&mut self, path: PathBuf, tab_index: usize) {
        if tab_index == 0 {
            self.previewed_left = Some(path);
        } else {
            self.previewed_right = Some(path);
        }
    }

    pub fn hide_preview(&mut self, tab_index: usize, ueberzug: &Ueberzug) {
        let preview_to_hide = if tab_index == 0 {
            &self.previewed_left
        } else {
            &self.previewed_right
        };
        let Some(path) = preview_to_hide else {
            return;
        };
        let Some(preview) = self.get(path) else {
            return;
        };
        preview.hide(ueberzug)
    }

    pub fn is_previewing(&self, tab_index: usize) -> &Option<PathBuf> {
        if tab_index == 0 {
            &self.previewed_left
        } else {
            &self.previewed_right
        }
    }

    /// Execute the preview creation and add it to the collection.
    /// It calls a static method which is sent to a tread in the threadpool.
    fn execute_preview_task(
        &self,
        previews: Arc<RwLock<BTreeMap<PathBuf, Arc<Preview>>>>,
        path: PathBuf,
        users: Users,
        should_build_thumbnail: bool,
    ) {
        self.pool.execute(move || {
            Self::build_and_store_preview(&previews, path, &users, should_build_thumbnail);
        });
    }

    /// Creates a preview and store it.
    /// Shouldn't be called directly but sent to the thread pool with an execute method.
    fn build_and_store_preview(
        previews: &Arc<RwLock<BTreeMap<PathBuf, Arc<Preview>>>>,
        path: PathBuf,
        users: &Users,
        should_build_thumbnail: bool,
    ) {
        if previews.read().contains_key(&path) {
            return;
        }
        let Ok(preview) = Preview::new(&path, users) else {
            log_info!("Couldn't build preview for {path}", path = path.display());
            return;
        };
        if should_build_thumbnail {
            if let Preview::Ueberzug(ueb) = &preview {
                let _ = ueb.build_thumbnail();
            };
        }
        log_info!("inserted {p} in preview_holder", p = path.display());
        previews.write().insert(path, Arc::new(preview));
    }

    /// Buid and store a preview for a single file. Does nothing if the preview already exists.
    /// If there's already too much previews, it will clear them first.
    pub fn build_single(&mut self, path: &Path) {
        if self.previews.read().contains_key(path) {
            return;
        }
        if self.previews.read().len() >= Self::MAX_PREVIEWS {
            self.previews.write().clear()
        }
        let previews = Arc::clone(&self.previews);
        let users = self.users.clone();
        let path = path.to_owned();
        self.execute_preview_task(previews, path, users, true);
    }

    /// Build and store a preview for multiple files.
    /// Clear the collection first since it should be called when changing directory.
    pub fn build_collection(&mut self, paths: Vec<PathBuf>) {
        self.clear();
        let mut thumbnail_counter = 10;
        for path in paths.into_iter().take(Self::MAX_PREVIEWS) {
            let previews = self.previews.clone();
            let users = self.users.clone();
            self.execute_preview_task(previews, path, users, thumbnail_counter >= 0);
            thumbnail_counter -= 1;
        }
    }
}

/// Simple threadpool which sends job to threads through mpsc.
/// Doesn't hold a joinhandle so it be Sync.
/// inspired by https://doc.rust-lang.org/book/ch20-02-multithreaded.html
#[derive(Clone)]
struct ThreadPool {
    sender: mpsc::Sender<Job>,
}

impl ThreadPool {
    /// Creates `size` threads with a mpsc receiver.
    fn new(size: usize) -> Self {
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            worker(id, Arc::clone(&receiver));
            workers.push(());
        }
        Self { sender }
    }

    /// Send the closure `f` to threads through the mpsc.
    /// The first available thread will do the job.
    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).unwrap();
    }
}

/// Boxed closure to be executed by a thread.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// Simple job executor in a thread.
/// reads a job from mpsc and execute it.
fn worker(_id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) {
    thread::spawn(move || loop {
        let Ok(job) = receiver.lock().recv() else {
            break;
        };
        job();
    });
}
