use std::ffi::{c_char, CString};
use std::sync::mpsc;
use std::thread;
use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use libloading::{Library, Symbol};

use crate::{
    log_info,
    modes::{Preview, PreviewBuilder},
};

enum PreviewRequest {
    Request((PathBuf, usize)),
    Quit,
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
    tx_request: mpsc::Sender<PreviewRequest>,
}

impl Previewer {
    /// Starts the previewer loop in a thread and create a new instance with a [`std::sync::mpsc::Sender`].
    ///
    /// The previewer will wait for [`std::sync::mpsc::Receiver`] messages and react accordingly :
    /// - if the message asks to quit, it will break the loop and leave.
    /// - if the message is a request, it will create the associate preview and send it back to the application.
    ///   The application should then ask the status to attach the preview. It's complicated but I couldn't find a simpler way to check
    ///   for the preview.
    pub fn new(tx_preview: mpsc::Sender<(PathBuf, Preview, usize)>) -> Self {
        let plugins = Self::load_previewer_plugins();
        let (tx_request, rx_request) = mpsc::channel::<PreviewRequest>();
        thread::spawn(move || {
            while let Some(request) = rx_request.iter().next() {
                match request {
                    PreviewRequest::Request((path, index)) => {
                        log_info!(
                            "Previewer: request for {path} tab {index}",
                            path = path.display()
                        );
                        if let Some(output) = Self::check_matchs(&path, &plugins) {
                            log_info!("plugin preview output:\n{output}");
                            let preview = PreviewBuilder::raw_text(output);
                            tx_preview.send((path, preview, index)).unwrap();
                        } else if let Ok(preview) = PreviewBuilder::new(&path).build() {
                            log_info!(
                                "Previewer: built preview for {path} tab {index}",
                                path = path.display()
                            );
                            tx_preview.send((path, preview, index)).unwrap();
                        };
                    }
                    PreviewRequest::Quit => break,
                }
            }
        });
        Self { tx_request }
    }

    /// Sends a "quit" message to the previewer loop. It will break the loop, exiting the previewer.
    pub fn quit(&self) {
        crate::log_info!("stopping previewer loop");
        match self.tx_request.send(PreviewRequest::Quit) {
            Ok(()) => (),
            Err(e) => crate::log_info!("Previewer::quit error {e:?}"),
        };
    }

    /// Sends an "ask preview" to the previewer loop. A preview will be built, which won't block the application.
    /// Once the preview is built, it's send back to status, which should be asked to attach the preview.
    /// The preview won't be attached automatically, it's the responsability of the application to do it.
    pub fn build(&self, path: PathBuf, index: usize) -> Result<()> {
        self.tx_request
            .send(PreviewRequest::Request((path, index)))?;
        Ok(())
    }

    fn load_previewer_plugins() -> HashMap<String, PreviewerPlugin> {
        let mut plugins = HashMap::new();
        plugins.insert("bat previewer".to_string(), Self::load_plugin("/home/quentin/gclem/dev/rust/fm/plugins/bat_previewer/target/release/libbat_previewer.so".to_string()));
        plugins
    }

    fn load_plugin(path: String) -> PreviewerPlugin {
        let lib = unsafe { Library::new(&path).expect("Couldn't load lib") };
        let name = unsafe {
            let name_fn: Symbol<extern "C" fn() -> *mut c_char> =
                lib.get(b"name").expect("Couldn't find name");
            let c_name = (name_fn)();
            if !c_name.is_null() {
                CString::from_raw(c_name)
                    .into_string()
                    .expect("Couldn't read name")
            } else {
                "".to_owned()
            }
        };
        let extensions = unsafe {
            let extensions_fn: Symbol<extern "C" fn() -> *mut c_char> =
                lib.get(b"extensions").expect("Couldn't find extensions");
            let c_extensions = (extensions_fn)();
            if !c_extensions.is_null() {
                CString::from_raw(c_extensions)
                    .into_string()
                    .expect("Couldn't read extensions")
            } else {
                "".to_owned()
            }
        };
        let previewer = unsafe {
            let previewer: Symbol<unsafe extern "C" fn(*mut c_char) -> *mut c_char> =
                lib.get(b"preview").expect("Couldn't find previewer");
            previewer.into_raw()
        };
        PreviewerPlugin {
            _lib: lib,
            name,
            extensions,
            previewer: *previewer,
        }
    }

    fn check_matchs(
        path: &std::path::Path,
        plugins: &HashMap<String, PreviewerPlugin>,
    ) -> Option<String> {
        let path_ext = path.extension()?.to_string_lossy().to_string();
        for plugin in plugins.values() {
            let extensions = &plugin.extensions;
            for ext in extensions.split_whitespace() {
                if ext == path_ext {
                    log_info!("Found match for {path_ext} from {name}", name = plugin.name);
                    let preview = unsafe {
                        let c_path = CString::new(path.display().to_string())
                            .expect("Couldn't create new string")
                            .into_raw();
                        log_info!("sending {c_path:?}");
                        let output = (plugin.previewer)(c_path);
                        CString::from_raw(output)
                            .into_string()
                            .expect("Couldn't convert preview output")
                    };
                    return Some(preview);
                }
            }
        }

        None
    }
}

#[derive(Debug)]
struct PreviewerPlugin {
    _lib: Library,
    name: String,
    extensions: String,
    previewer: unsafe extern "C" fn(*mut c_char) -> *mut c_char,
}
