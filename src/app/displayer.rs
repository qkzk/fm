use std::io::Stdout;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::Status;
use crate::io::Display;
use crate::log_info;

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
                        crate::log_info!("terminating displayer");
                        let _ = display.show_cursor();
                        display.restore_terminal()?;
                        drop(display);
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
                match status.lock() {
                    Ok(status) => {
                        display.display_all(&status);
                        drop(status);
                    }
                    Err(error) => bail!("Error locking status: {error}"),
                }
                std::thread::sleep(Duration::from_millis(Self::THIRTY_PER_SECONDS_IN_MILLIS));
            }
            Ok(())
        });
        Self { tx, handle }
    }

    pub fn quit(self) {
        crate::log_info!("stopping display loop");
        match self.tx.send(()) {
            Ok(()) => (),
            Err(e) => log_info!("Displayer::quit error {e:?}"),
        };
        let _ = self.handle.join();
    }
}
