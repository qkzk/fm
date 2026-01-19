use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;

use crate::modes::{Preview, PreviewBuilder};

enum RequestKind {
    PreviewRequest(PreviewRequest),
    Quit,
}

struct PreviewRequest {
    path: PathBuf,
    tab_index: usize,
    line_nr: Option<usize>,
}

impl PreviewRequest {
    fn new(path: PathBuf, tab_index: usize, line_nr: Option<usize>) -> Self {
        Self {
            path,
            tab_index,
            line_nr,
        }
    }
}

/// Response from the preview builder with the request data it came from.
/// The path and tab index are used to ensure we're attaching the correct preview for this tab,
/// The line number, if any, is used to scroll the the line. Only fuzzy picker of line may output a line number.
pub struct PreviewResponse {
    pub path: PathBuf,
    pub tab_index: usize,
    pub line_nr: Option<usize>,
    pub preview: Preview,
}

impl PreviewResponse {
    fn new(path: PathBuf, tab_index: usize, line_nr: Option<usize>, preview: Preview) -> Self {
        Self {
            path,
            tab_index,
            line_nr,
            preview,
        }
    }
}

/// Non blocking preview builder.
///
/// Allow preview building without blocking status.
/// The process is quite complicated but fast.
/// We use 2 [`std::sync::mpsc`] :
/// - one to ask a preview to be built, sent from [`crate::app::Status`] itself, it's received here in a separated thread and built outisde of status thread.
/// - one to send the [`crate::modes::Preview`] out of the thread to status. It's the responsability of the application to force status to attach the preview.
///
/// ATM only the previews for the second pane are built here. It's useless if the user display previews in the current tab since navigation isn't possible.
pub struct Previewer {
    tx_request: mpsc::Sender<RequestKind>,
}

impl Previewer {
    /// Starts the previewer loop in a thread and create a new instance with a [`std::sync::mpsc::Sender`].
    ///
    /// The previewer will wait for [`std::sync::mpsc::Receiver`] messages and react accordingly :
    /// - if the message asks to quit, it will break the loop and leave.
    /// - if the message is a request, it will create the associate preview and send it back to the application.
    ///   The application should then ask the status to attach the preview. It's complicated but I couldn't find a simpler way to check
    ///   for the preview.
    pub fn new(tx_preview: mpsc::Sender<PreviewResponse>) -> Self {
        let (tx_request, rx_request) = mpsc::channel::<RequestKind>();
        thread::spawn(move || {
            while let Some(request) = rx_request.iter().next() {
                match request {
                    RequestKind::PreviewRequest(PreviewRequest {
                        path,
                        tab_index,
                        line_nr,
                    }) => {
                        if let Ok(preview) = PreviewBuilder::new(&path).build() {
                            tx_preview
                                .send(PreviewResponse::new(path, tab_index, line_nr, preview))
                                .unwrap();
                        };
                    }
                    RequestKind::Quit => break,
                }
            }
        });
        Self { tx_request }
    }

    /// Sends a "quit" message to the previewer loop. It will break the loop, exiting the previewer.
    pub fn quit(&self) {
        crate::log_info!("stopping previewer loop");
        match self.tx_request.send(RequestKind::Quit) {
            Ok(()) => (),
            Err(e) => crate::log_info!("Previewer::quit error {e:?}"),
        };
    }

    /// Sends an "ask preview" to the previewer loop. A preview will be built, which won't block the application.
    /// Once the preview is built, it's send back to status, which should be asked to attach the preview.
    /// The preview won't be attached automatically, it's the responsability of the application to do it.
    pub fn build(&self, path: PathBuf, index: usize, line_index: Option<usize>) -> Result<()> {
        self.tx_request
            .send(RequestKind::PreviewRequest(PreviewRequest::new(
                path, index, line_index,
            )))?;
        Ok(())
    }
}
