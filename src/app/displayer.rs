use std::io::Stdout;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use parking_lot::Mutex;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::Status;
use crate::io::Display;
use crate::log_info;

/// Is responsible for running the display thread.
/// Rendering is done at 30 fps if possible.
/// It holds a transmitter used to ask the thread to stop and an handle to this thread.
/// It's usefull to ensure the terminal is reset properly which should always be the case.
pub struct Displayer {
    tx: mpsc::Sender<()>,
    handle: thread::JoinHandle<Result<()>>,
}

impl Displayer {
    const THIRTY_PER_SECONDS_IN_MILLIS: u64 = 33;

    pub fn new(term: Terminal<CrosstermBackend<Stdout>>, status: Arc<Mutex<Status>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut display = Display::new(term);

        let handle = thread::spawn(move || -> Result<()> {
            loop {
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        log_info!("terminating displayer");
                        display.restore_terminal()?;
                        drop(display);
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
                let mut status = status.lock();
                if !status.internal_settings.is_disabled() {
                    display.display_all(&status);
                }
                if status.should_tabs_images_be_cleared() {
                    status.set_tabs_images_cleared();
                }
                if status.should_be_cleared() {
                    status.internal_settings.reset_clear()
                }
                drop(status);

                thread::sleep(Duration::from_millis(Self::THIRTY_PER_SECONDS_IN_MILLIS));
            }
            Ok(())
        });
        Self { tx, handle }
    }

    pub fn quit(self) {
        log_info!("stopping display loop");
        match self.tx.send(()) {
            Ok(()) => (),
            Err(e) => log_info!("Displayer::quit error {e:?}"),
        };
        let _ = self.handle.join();
    }
}
