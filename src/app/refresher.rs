use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::event::{create_stream, read_from_stream, FmEvents};
use crate::log_info;

/// Allows refresh if the current path has been modified externally.
pub struct Refresher {
    /// Sender of messages, used to terminate the thread properly
    tx: mpsc::Sender<()>,
    /// Handle to the `term::Event` sender thread.
    handle: thread::JoinHandle<()>,
}

impl Refresher {
    /// Between 2 refreshed
    const TEN_SECONDS_IN_CENTISECONDS: u16 = 10 * 100;

    /// Event sent to Fm event poller which is interpreted
    /// as a request for refresh.

    /// Spawn a thread which sends events to the terminal.
    /// Those events are interpreted as refresh requests.
    /// It also listen to a receiver for quit messages.
    pub fn new(fm_sender: Arc<Sender<FmEvents>>) -> Self {
        let (tx, rx) = mpsc::channel();
        // let mut counter: u16 = 0;
        let mut counter = Counter::default();
        let (socket_path, socket_listener) = create_stream().expect("Error creating stream");
        socket_listener
            .set_nonblocking(true)
            .expect("Couldn't set socket to non blocking");
        let handle = thread::spawn(move || loop {
            // TODO: This isn't the responsability of refresher to transfer events from the socket.
            // TODO: either create anoter thread for that or do it cleaner.
            // TODO: too much send there should be only one in the whole closure.
            if let Ok((mut stream, path)) = socket_listener.accept() {
                crate::log_info!("Accepted socket connection from {path:?}");
                if let Some(msg) = read_from_stream(&mut stream) {
                    let event = FmEvents::Ipc(msg);
                    if fm_sender.send(event).is_err() {
                        remove_socket(&socket_path);
                        break;
                    }
                }
            }
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    remove_socket(&socket_path);
                    return;
                }
                Err(TryRecvError::Empty) => {}
            }
            counter.incr();
            thread::sleep(Duration::from_millis(10));
            let event = counter.pick_according_to_counter();
            if fm_sender.send(event).is_err() {
                remove_socket(&socket_path);
                break;
            }
        });
        Self { tx, handle }
    }

    /// Send a quit message to the receiver, signaling it to quit.
    /// Join the refreshing thread which should be terminated.
    pub fn quit(self) {
        let _ = self.tx.send(());
        let _ = self.handle.join();
    }
}

fn remove_socket(socket_path: &str) {
    std::fs::remove_file(socket_path).expect("Couldn't delete socket file");
    log_info!("Deleted socket {socket_path}");
    log_info!("terminating refresher");
}

#[derive(Default)]
struct Counter {
    counter: u16,
}

impl Counter {
    fn pick_according_to_counter(&mut self) -> FmEvents {
        if self.counter >= Refresher::TEN_SECONDS_IN_CENTISECONDS {
            self.counter = 0;
            FmEvents::Refresh
        } else {
            FmEvents::UpdateTick
        }
    }

    fn incr(&mut self) {
        self.counter += 1;
    }
}
