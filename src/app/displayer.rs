use crate::io::Display;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::app::Status;

pub struct Displayer {
    tx: mpsc::Sender<()>,
    handle: thread::JoinHandle<Result<()>>,
}

impl Displayer {
    pub fn new(term: Arc<tuikit::term::Term>, status: Arc<Mutex<Status>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut display = Display::new(term);

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
                let status = status.lock().unwrap();
                display.display_all(&status)?;
                drop(status);

                std::thread::sleep(Duration::from_millis(17));
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
