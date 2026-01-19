use std::{os::unix::net::UnixListener, sync::mpsc::Receiver, time::Duration};

use crate::event::{create_stream, read_from_stream, FmEvents};

/// Simple struct to read the events.
pub struct EventReader {
    pub fm_receiver: Receiver<FmEvents>,
    pub socket_path: String,
    pub socket_listener: UnixListener,
}

impl EventReader {
    /// Creates a new instance with an Arc to a terminal.
    pub fn new(fm_receiver: Receiver<FmEvents>) -> Self {
        let (socket_path, socket_listener) = create_stream().expect("Error creating stream");
        socket_listener
            .set_nonblocking(true)
            .expect("Couldn't set socket to non blocking");
        Self {
            fm_receiver,
            socket_path,
            socket_listener,
        }
    }

    const POLL_WAIT_MS: Duration = Duration::from_millis(20);

    /// Returns the events as they're received. Loops until an event is received.
    /// We should spend most of the application life here, polling for next event :)
    ///
    /// It's an interface for internal and extenal events (through terminal)
    /// casting them into an [`crate::event::FmEvents`].
    pub fn poll_event(&self) -> FmEvents {
        loop {
            if let Ok((mut stream, path)) = self.socket_listener.accept() {
                crate::log_info!("Accepted socket connection from {path:?}");
                if let Some(msg) = read_from_stream(&mut stream) {
                    return FmEvents::Ipc(msg);
                }
            }
            if let Ok(event) = self.fm_receiver.try_recv() {
                return event;
            }
            let Ok(true) = crossterm::event::poll(Self::POLL_WAIT_MS) else {
                continue;
            };
            let Ok(term_event) = crossterm::event::read() else {
                continue;
            };
            return FmEvents::Term(term_event);
        }
    }
}
