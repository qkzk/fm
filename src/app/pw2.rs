/*

# First step

1. pool de workers
2. queue
    - enqueue
    - clear
    - dequeue

queue.enqueue(task)
    ajoute la task a la queue

queue.clear()
    vide la queue

queue.dequeue() -> task
    renvoie le plus ancier

worker
    spawn
        receive Arc<Mutex<Queue>>
    start
        loop
            ask the queue for a task
                lock the queue
                queue.dequeue() -> task
                unlock the queue
            preview(task)

status has Arc<Mutex<Queue>>
    add_queue(task)
        lock the queue
        enqueue
        unlock the queue

    clear_preview_queue()
        lock the queue
        clear the queue
        unlock the queue

# Second step: follow cd & clear
# Thrid use it for ueberzug
# Third step: quit gracefully, require mpsc for worker

*/
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::log_info;
use crate::modes::PreviewBuilder;

fn make_preview<P>(path: P)
where
    P: AsRef<std::path::Path>,
{
    match PreviewBuilder::new(path.as_ref()).build() {
        Ok(_preview) => log_info!("preview built successfully"),
        Err(e) => log_info!("error building preview {e}"),
    }
}

#[derive(Debug)]
pub struct PreviewManager {
    queue: Arc<Mutex<Vec<PathBuf>>>,
    workers: Vec<Worker>,
}

impl Default for PreviewManager {
    fn default() -> Self {
        let num_workers = Self::default_thread_count().unwrap_or(1);
        let queue = Arc::new(Mutex::new(vec![]));
        let mut workers = Vec::with_capacity(num_workers);
        for id in 0..num_workers {
            workers.push(Worker::new(id, queue.clone()));
        }

        Self { queue, workers }
    }
}

impl PreviewManager {
    fn default_thread_count() -> Option<usize> {
        thread::available_parallelism()
            .map(|it| it.get().checked_sub(6).unwrap_or(1))
            .ok()
    }

    pub fn enqueue<P: AsRef<Path>>(&self, task: P) {
        let Ok(mut locked_queue) = self.queue.lock() else {
            log_info!("PreviewManager couldn't lock the queue");
            return;
        };
        log_info!("PreviewManager add {p}", p = task.as_ref().display());
        locked_queue.push(task.as_ref().to_path_buf());
        drop(locked_queue);
    }

    pub fn collection<P: AsRef<Path>>(&self, tasks: &[P]) {
        let Ok(mut locked_queue) = self.queue.lock() else {
            log_info!("PreviewManager couldn't lock the queue");
            return;
        };
        log_info!("PreviewManager collection");
        for task in tasks {
            locked_queue.push(task.as_ref().to_path_buf());
        }
        drop(locked_queue);
    }

    pub fn clear(&self) {
        let Ok(mut locked_queue) = self.queue.lock() else {
            return;
        };
        locked_queue.clear();
        drop(locked_queue);
    }
}

#[derive(Debug)]
pub struct Worker {
    id: usize,
}

impl Worker {
    pub fn new(id: usize, queue: Arc<Mutex<Vec<PathBuf>>>) -> Self {
        let worker = Self { id };
        let _join_handle = thread::spawn(move || loop {
            let Ok(mut locked_queue) = queue.lock() else {
                log_info!("Worker {id} couldn't lock the queue");
                continue;
            };

            if !locked_queue.is_empty() {
                let task = locked_queue.remove(0);
                log_info!("Worker {id} received task {p}", p = task.display());
                make_preview(task);
            }
            drop(locked_queue);

            thread::sleep(Duration::from_millis(10));
        });
        log_info!("Worker {id} started");

        worker
    }
}
