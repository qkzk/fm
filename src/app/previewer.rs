use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;

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
        let plugins = previewer_plugins::load_previewer_plugins();
        let (tx_request, rx_request) = mpsc::channel::<PreviewRequest>();
        thread::spawn(move || {
            while let Some(request) = rx_request.iter().next() {
                match request {
                    PreviewRequest::Request((path, index)) => {
                        if let Some(preview) = previewer_plugins::check_matchs(&path, &plugins) {
                            tx_preview.send((path, preview, index)).unwrap();
                        } else if let Ok(preview) = PreviewBuilder::new(&path).build() {
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
}

/// TODO: move elesewere
mod previewer_plugins {
    use std::{
        collections::HashMap,
        ffi::{c_char, CString},
    };

    use libloading::{Library, Symbol};

    use crate::modes::{Preview, PreviewBuilder};

    pub fn load_previewer_plugins() -> HashMap<String, PreviewerPlugin> {
        let mut plugins = HashMap::new();
        // TODO: get from config file
        plugins.insert("bat previewer".to_string(), load_plugin("/home/quentin/gclem/dev/rust/fm/plugins/bat_previewer/target/release/libbat_previewer.so".to_string()));
        plugins
    }

    fn load_plugin(path: String) -> PreviewerPlugin {
        let lib = unsafe { get_lib(path) };
        let name = unsafe { get_name(&lib) };
        let extensions = unsafe { get_extensions(&lib) };
        let previewer = unsafe { get_previewer(&lib).into_raw() };
        PreviewerPlugin {
            _lib: lib,
            name,
            extensions,
            previewer: *previewer,
        }
    }

    unsafe fn get_lib(path: String) -> Library {
        Library::new(&path).expect("Couldn't load lib")
    }

    unsafe fn get_name(lib: &Library) -> String {
        let name_fn: Symbol<extern "C" fn() -> *mut c_char> =
            unsafe { lib.get(b"name").expect("Couldn't find name") };
        let c_name = (name_fn)();
        if !c_name.is_null() {
            unsafe {
                CString::from_raw(c_name)
                    .into_string()
                    .expect("Couldn't read name")
            }
        } else {
            "".to_owned()
        }
    }

    unsafe fn get_extensions(lib: &Library) -> String {
        let extensions_fn: Symbol<extern "C" fn() -> *mut c_char> =
            unsafe { lib.get(b"extensions").expect("Couldn't find extensions") };
        let c_extensions = (extensions_fn)();
        if !c_extensions.is_null() {
            unsafe {
                CString::from_raw(c_extensions)
                    .into_string()
                    .expect("Couldn't read extensions")
            }
        } else {
            "".to_owned()
        }
    }

    unsafe fn get_previewer(
        lib: &Library,
    ) -> Symbol<unsafe extern "C" fn(*mut c_char) -> *mut c_char> {
        lib.get(b"preview").expect("Couldn't find previewer")
    }

    pub fn check_matchs(
        path: &std::path::Path,
        plugins: &HashMap<String, PreviewerPlugin>,
    ) -> Option<Preview> {
        let path_ext = path.extension()?.to_string_lossy().to_string();
        for plugin in plugins.values() {
            let extensions = &plugin.extensions;
            for ext in extensions.split_whitespace() {
                if ext == path_ext {
                    let c_path = CString::new(path.display().to_string())
                        .expect("Couldn't create new string")
                        .into_raw();
                    let output = unsafe { plugin.get_output(c_path) };
                    return Some(PreviewBuilder::plugin_text(output, &plugin.name));
                }
            }
        }

        None
    }

    #[derive(Debug)]
    pub struct PreviewerPlugin {
        _lib: Library,
        name: String,
        extensions: String,
        previewer: unsafe extern "C" fn(*mut c_char) -> *mut c_char,
    }

    impl PreviewerPlugin {
        unsafe fn get_output(&self, c_path: *mut c_char) -> String {
            let output = (self.previewer)(c_path);
            CString::from_raw(output)
                .into_string()
                .expect("Couldn't convert preview output")
        }
    }
}
