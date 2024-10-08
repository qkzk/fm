use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::event::FmEvents;
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
    const TEN_SECONDS_IN_DECISECONDS: u8 = 10 * 10;

    /// Event sent to Fm event poller which is interpreted
    /// as a request for refresh.
    /// This key can't be bound to anything (who would use that ?).

    /// Spawn a thread which sends events to the terminal.
    /// Those events are interpreted as refresh requests.
    /// It also listen to a receiver for quit messages.
    ///
    /// Using Event::User(()) conflicts with skim internal which interpret this
    /// event as a signal(1) and hangs the terminal.
    pub fn new(fm_sender: Arc<Sender<FmEvents>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut counter: u8 = 0;
        let handle = thread::spawn(move || loop {
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    log_info!("terminating refresher");
                    // let _ = term.show_cursor(true);
                    return;
                }
                Err(TryRecvError::Empty) => {}
            }
            counter += 1;
            thread::sleep(Duration::from_millis(100));
            if counter >= Self::TEN_SECONDS_IN_DECISECONDS {
                counter = 0;
                if fm_sender.send(FmEvents::Refresh).is_err() {
                    // if term.send_event(REFRESH_EVENT).is_err() {
                    break;
                }
            }
        });
        Self { tx, handle }
    }

    /// Send a quit message to the receiver, signaling it to quit.
    /// Join the refreshing thread which should be terminated.
    pub fn quit(self) {
        match self.tx.send(()) {
            Ok(_) => (),
            Err(e) => log_info!("Refresher::quit error {e:?}"),
        };
        let _ = self.handle.join();
    }
}
