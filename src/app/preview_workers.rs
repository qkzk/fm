use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use crate::log_info;
use crate::modes::PreviewBuilder;

/// The worker that executes tasks from the queue.
struct PreviewWorker {
    id: usize,
    task_receiver: mpsc::Receiver<PathBuf>,
}

impl PreviewWorker {
    fn new(id: usize, task_receiver: mpsc::Receiver<PathBuf>) -> Self {
        log_info!("spawning worker {id}");
        Self { id, task_receiver }
    }

    fn run(self) {
        while let Ok(filepath) = self.task_receiver.recv() {
            log_info!("Worker {} processing task: {:?}", self.id, filepath);
            make_preview(filepath);
        }
        log_info!("Worker {} exiting.", self.id);
    }
}

/// Placeholder for the `make_preview` function.
fn make_preview<P>(path: P)
where
    P: AsRef<std::path::Path>,
{
    match PreviewBuilder::new(path.as_ref()).build() {
        Ok(_preview) => log_info!("preview built successfully"),
        Err(e) => log_info!("error building preview {e}"),
    }
}

/// Manages the queue of tasks and workers.
struct PreviewQueue {
    task_senders: Vec<mpsc::Sender<PathBuf>>,
    curr_worker: usize,
    num_workers: usize,
}

impl PreviewQueue {
    fn new(num_workers: usize) -> Self {
        Self {
            task_senders: Self::build_task_senders(num_workers),
            curr_worker: 0,
            num_workers,
        }
    }

    fn build_task_senders(num_workers: usize) -> Vec<mpsc::Sender<PathBuf>> {
        let mut task_senders = Vec::with_capacity(num_workers);

        for id in 0..num_workers {
            let (task_sender, task_receiver) = mpsc::channel(); // Buffer size of 10 tasks per worker

            // Spawn each worker task
            thread::spawn(move || {
                let worker = PreviewWorker::new(id, task_receiver);
                worker.run()
            });

            task_senders.push(task_sender);
        }
        task_senders
    }

    fn add_task(&mut self, filepath: impl AsRef<Path>) {
        let task = filepath.as_ref().to_path_buf();

        // for sender in &self.task_senders {

        // Send the task to each worker in a round-robin fashion
        if let Err(err) = self.task_senders[self.curr_worker].send(task) {
            log_info!(
                "Failed to send task: {err} for {curr} - {fp}",
                curr = self.curr_worker,
                fp = filepath.as_ref().display()
            );
        }

        self.curr_worker = (self.curr_worker + 1) % self.num_workers;
    }

    /// Clears all pending tasks in the queue.
    pub fn clear_queue(&mut self) {
        // Dropping the sender will close the channel
        let _ = std::mem::take(&mut self.task_senders);
        self.task_senders = Self::build_task_senders(self.num_workers);
        self.curr_worker = 0;
    }

    // /// Shuts down all workers.
    // async fn stop_workers(&self) {
    //     drop(self.task_sender.clone()); // Dropping the sender will close the channel
    //     for worker in &self.workers {
    //         worker.abort(); // This stops all worker tasks
    //     }
    // }
}

/// The main manager for controlling the preview system.
pub struct PreviewManager {
    queue: PreviewQueue,
}

impl Default for PreviewManager {
    fn default() -> Self {
        let num_workers = Self::default_thread_count().unwrap();
        log_info!("PreviewManager started. Workers are ready to process tasks.");
        Self {
            queue: PreviewQueue::new(num_workers),
        }
    }
}

impl PreviewManager {
    fn default_thread_count() -> Option<usize> {
        thread::available_parallelism()
            .map(|it| it.get().checked_sub(2).unwrap_or(1))
            .ok()
    }

    /// Adds a preview task to the queue.
    pub fn add_preview_task(&mut self, filepath: impl AsRef<Path>) {
        self.queue.add_task(&filepath);
        log_info!("added {p}", p = filepath.as_ref().display());
    }

    // /// Clears all pending tasks in the queue.
    // pub async fn clear_tasks(&self) {
    //     queue.clear_queue().await;
    // }

    // /// Stops all workers gracefully.
    // pub async fn stop(&self) {
    //     let queue = self.queue.lock().await;
    //     queue.stop_workers().await;
    // }
}

pub fn build_preview_manager() -> PreviewManager {
    PreviewManager::default()
}
