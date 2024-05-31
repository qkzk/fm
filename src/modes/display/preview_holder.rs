use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;

use parking_lot::{Mutex, RwLock};

use crate::log_info;
use crate::modes::Preview;
use crate::modes::Ueberzug;
use crate::modes::Users;

#[derive(Clone)]
pub struct PreviewHolder {
    pub previews: Arc<RwLock<BTreeMap<PathBuf, Arc<Preview>>>>,
    pool: ThreadPool,
    users: Users,
}

impl PreviewHolder {
    const MAX_PREVIEWS: usize = 500;
    const NB_WORKERS: usize = 4;

    pub fn new() -> Self {
        let users = Users::new();
        let previews = Arc::new(RwLock::new(BTreeMap::new()));
        let pool = ThreadPool::new(Self::NB_WORKERS);
        Self {
            previews,
            pool,
            users,
        }
    }

    pub fn get(&self, p: &Path) -> Option<Arc<Preview>> {
        self.previews.read().get(p).cloned()
    }

    fn clear(&mut self) {
        self.previews.write().clear();
    }

    pub fn put_preview<P>(&mut self, path: P, preview: Preview)
    where
        P: AsRef<Path>,
    {
        self.previews
            .write()
            .insert(path.as_ref().to_owned(), Arc::new(preview));
    }

    fn execute_preview_task(
        &self,
        previews: Arc<RwLock<BTreeMap<PathBuf, Arc<Preview>>>>,
        path: PathBuf,
        users: Users,
        ueberzug: Arc<Ueberzug>,
    ) {
        self.pool.execute(move || {
            Self::build_and_store_preview(&previews, path, &users, ueberzug);
        });
    }

    fn build_and_store_preview(
        previews: &Arc<RwLock<BTreeMap<PathBuf, Arc<Preview>>>>,
        path: PathBuf,
        users: &Users,
        ueberzug: Arc<Ueberzug>,
    ) {
        if previews.read().contains_key(&path) {
            return;
        }
        let Ok(preview) = Preview::new(&path, users, ueberzug) else {
            log_info!("Couldn't build preview for {path}", path = path.display());
            return;
        };
        log_info!("inserted {p} in preview_holder", p = path.display());
        previews.write().insert(path, Arc::new(preview));
    }

    pub fn build_single(&mut self, path: &Path, ueberzug: Arc<Ueberzug>) {
        if self.previews.read().contains_key(path) {
            return;
        }
        if self.previews.read().len() >= Self::MAX_PREVIEWS {
            self.previews.write().clear()
        }
        let previews = Arc::clone(&self.previews);
        let users = self.users.clone();
        let path = path.to_owned();
        self.execute_preview_task(previews, path, users, ueberzug);
    }

    pub fn build_collection(&mut self, paths: Vec<PathBuf>, ueberzug: &Arc<Ueberzug>) {
        self.clear();
        for path in paths.into_iter().take(Self::MAX_PREVIEWS) {
            let previews = self.previews.clone();
            let users = self.users.clone();
            let ueberzug = ueberzug.clone();
            self.execute_preview_task(previews, path, users, ueberzug);
        }
    }

    pub fn hide_all_images(&mut self) {
        self.previews
            .read()
            .values()
            .for_each(|preview| preview.hide())
    }
}

#[derive(Clone)]
struct ThreadPool {
    sender: mpsc::Sender<Job>,
}

impl ThreadPool {
    fn new(size: usize) -> Self {
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);

        for id in 0..size {
            workers.push(worker(id, Arc::clone(&receiver)));
        }
        Self { sender }
    }

    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.send(job).unwrap();
    }
}

type Job = Box<dyn FnOnce() + Send + 'static>;

fn worker(_id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) {
    thread::spawn(move || loop {
        let job = receiver.lock().recv().unwrap();
        job();
    });
}
