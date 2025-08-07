use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;

use crate::modes::{Preview, PreviewBuilder};

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
        let (tx_request, rx_request) = mpsc::channel::<PreviewRequest>();
        thread::spawn(move || {
            while let Some(request) = rx_request.iter().next() {
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

/// TODO: move elesewhere
pub mod previewer_plugins {
    use std::{
        collections::HashMap,
        ffi::{c_char, CString},
    };

    use anyhow::{bail, Result};
    use libloading::{Library, Symbol};

    use crate::modes::{Preview, PreviewBuilder};

    // TODO: don't use hashmap since plugins are tested with `map.iter()` and it uses arbitrary order.
    // we must ensure the plugins are tested with the order the user provided

    /// Build an hashmap of name and preview builder from an hashmap of name and path.
    pub fn build_plugins(plugins: HashMap<String, String>) -> HashMap<String, PreviewerPlugin> {
        let mut loaded_plugins = HashMap::new();
        for (name, path) in plugins.into_iter() {
            let Some(loaded_plugin) = load_plugin(path) else {
                continue;
            };
            loaded_plugins.insert(name, loaded_plugin);
        }
        loaded_plugins
    }

    // TODO: make it a result allowing errors in log
    fn load_plugin(path: String) -> Option<PreviewerPlugin> {
        let _lib = unsafe { get_lib(path) }.ok()?;
        let name = unsafe { get_name(&_lib) }.ok()?;
        let is_match = unsafe { *(get_matcher(&_lib).ok()?) };
        let previewer = unsafe { *(get_previewer(&_lib)).ok()? };
        Some(PreviewerPlugin {
            _lib,
            name,
            is_match,
            previewer,
        })
    }

    unsafe fn get_lib(path: String) -> Result<Library, libloading::Error> {
        Library::new(&path)
    }

    unsafe fn get_name(lib: &Library) -> Result<String> {
        let name_fn: Symbol<extern "C" fn() -> *mut c_char> = unsafe { lib.get(b"name")? };
        let c_name = (name_fn)();
        if !c_name.is_null() {
            unsafe {
                return Ok(CString::from_raw(c_name).into_string()?);
            }
        }
        bail!("name string is null");
    }

    unsafe fn get_matcher(
        lib: &Library,
    ) -> Result<Symbol<unsafe extern "C" fn(*mut c_char) -> bool>, libloading::Error> {
        lib.get(b"is_match")
    }

    unsafe fn get_previewer(
        lib: &Library,
    ) -> Result<Symbol<unsafe extern "C" fn(*mut c_char) -> *mut c_char>, libloading::Error> {
        lib.get(b"preview")
    }

    /// Preview the file if any loaded plugin is able to.
    pub fn try_build(
        path: &std::path::Path,
        plugins: &HashMap<String, PreviewerPlugin>,
    ) -> Option<Preview> {
        let s_path = path.to_string_lossy().to_string();
        let candidate = CString::new(s_path).ok()?.into_raw();
        for plugin in plugins.values() {
            if unsafe { (plugin.is_match)(candidate) } {
                let c_path = CString::new(path.display().to_string()).ok()?.into_raw();
                let output = unsafe { plugin.get_output(c_path) }.ok()?;
                return Some(PreviewBuilder::plugin_text(output, &plugin.name));
            }
        }
        None
    }

    #[derive(Debug)]
    pub struct PreviewerPlugin {
        _lib: Library,
        name: String,
        is_match: unsafe extern "C" fn(*mut c_char) -> bool,
        previewer: unsafe extern "C" fn(*mut c_char) -> *mut c_char,
    }

    impl PreviewerPlugin {
        unsafe fn get_output(&self, c_path: *mut c_char) -> Result<String> {
            let output = (self.previewer)(c_path);
            Ok(CString::from_raw(output).into_string()?)
        }
    }
}
