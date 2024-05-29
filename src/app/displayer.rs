use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use parking_lot::RwLock;

use crate::app::Status;
use crate::io::Display;
use crate::modes::PreviewHolder;

pub struct Displayer {
    tx: mpsc::Sender<()>,
    handle: thread::JoinHandle<Result<()>>,
}

impl Displayer {
    const THIRTY_PER_SECONDS_IN_MILLIS: u64 = 33;

    pub fn new(
        term: Arc<tuikit::term::Term>,
        status: Arc<RwLock<Status>>,
        preview_holder: Arc<RwLock<PreviewHolder>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut display = Display::new(term, preview_holder);

        let handle = thread::spawn(move || -> Result<()> {
            loop {
                match rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => {
                        crate::log_info!("terminating displayer");
                        let _ = display.show_cursor();
                        drop(display);
                        break;
                    }
                    Err(TryRecvError::Empty) => {}
                }
                display.display_all(&status.read())?;
                std::thread::sleep(Duration::from_millis(Self::THIRTY_PER_SECONDS_IN_MILLIS));
            }
            Ok(())
        });
        Self { tx, handle }
    }

    pub fn quit(self) -> Result<()> {
        crate::log_info!("stopping display loop");
        self.tx.send(())?;
        let _ = self.handle.join();
        Ok(())
    }
}
