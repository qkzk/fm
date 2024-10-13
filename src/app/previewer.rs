use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;

use crate::modes::{Preview, PreviewBuilder};

enum PreviewRequest {
    Request((PathBuf, usize)),
    Quit,
}

pub struct Previewer {
    tx: mpsc::Sender<PreviewRequest>,
}

/// Non blocking preview builder.
///
/// Allow preview building without blocking status.
/// The process is quite complicated but quick.
/// We use 2 [`std::sync::mpsc`] :
/// - one to ask a preview to be built, sent from [`crate::app::Status`] itself, it's received here in a separated thread and built outisde of status thread.
/// - one to send the [`crate::modes::Preview`] out of the thread to status. It's the responsability of the application to force status to attach the preview.
impl Previewer {
    /// Starts the previewer loop in a thread and create a new instance with a [`std::sync::mpsc::Sender`].
    ///
    /// The previewer will wait for [`std::sync::mpsc::Receiver`] messages and react accordingly :
    /// - if the message asks to quit, it will break the loop and leave.
    /// - if the message is a request, it will create the associate preview and send it back to the application.
    ///   The application should then ask the status to attach the preview. It's complicated but I couldn't find a simpler way to check
    ///   for the preview.
    pub fn new(tx_preview: mpsc::Sender<(PathBuf, Preview, usize)>) -> Self {
        let (tx, rx) = mpsc::channel::<PreviewRequest>();
        thread::spawn(move || {
            while let Some(request) = rx.iter().next() {
                match request {
                    PreviewRequest::Request((path, index)) => {
                        if let Ok(preview) = PreviewBuilder::new(&path).build() {
                            tx_preview.send((path, preview, index)).unwrap();
                        };
                    }
                    PreviewRequest::Quit => break,
                }
            }
        });
        Self { tx }
    }

    /// Sends a "quit" message to the previewer loop. It will break the loop, exiting the previewer.
    pub fn quit(&self) {
        crate::log_info!("stopping previewer loop");
        match self.tx.send(PreviewRequest::Quit) {
            Ok(()) => (),
            Err(e) => crate::log_info!("Previewer::quit error {e:?}"),
        };
    }

    /// Sends an "ask preview" to the previewer loop. A preview will be built, which won't block the application.
    /// Once the preview is built, it's send back to status, which should be asked to attach the preview.
    /// The preview won't be attached automatically, it's the responsability of the application to do it.
    pub fn build(&self, path: PathBuf, index: usize) -> Result<()> {
        self.tx.send(PreviewRequest::Request((path, index)))?;
        Ok(())
    }
}
