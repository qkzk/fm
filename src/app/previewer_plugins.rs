use std::{
    ffi::{c_char, CString},
    path::Path,
};

use anyhow::{bail, Result};
use libloading::{Library, Symbol};

use crate::modes::{Preview, PreviewBuilder};

/// Build an hashmap of name and preview builder from an hashmap of name and path.
pub fn build_previewer_plugins(plugins: Vec<(String, String)>) -> Vec<(String, PreviewerPlugin)> {
    let mut loaded_plugins = vec![];
    for (name, path) in plugins.into_iter() {
        match load_plugin(path) {
            Ok(loaded_plugin) => loaded_plugins.push((name, loaded_plugin)),
            Err(error) => {
                crate::log_info!("Error loading plugin {error:?}");
                crate::log_line!("Plugin {name} couldn't be loaded. See logs.")
            }
        }
    }
    loaded_plugins
}

fn load_plugin(path: String) -> Result<PreviewerPlugin> {
    let _lib = unsafe { get_lib(path) }?;
    let name = unsafe { get_name(&_lib) }?;
    let is_match = unsafe { *(get_matcher(&_lib)?) };
    let previewer = unsafe { *(get_previewer(&_lib))? };
    Ok(PreviewerPlugin {
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
) -> Result<Symbol<'_, unsafe extern "C" fn(*mut c_char) -> bool>, libloading::Error> {
    lib.get(b"is_match")
}

unsafe fn get_previewer(
    lib: &Library,
) -> Result<Symbol<'_, unsafe extern "C" fn(*mut c_char) -> *mut c_char>, libloading::Error> {
    lib.get(b"preview")
}

/// Preview the file if any loaded plugin is able to.
pub fn try_build_plugin(path: &Path, plugins: &[(String, PreviewerPlugin)]) -> Option<Preview> {
    let s_path = path.to_string_lossy().to_string();
    for (_, plugin) in plugins.iter() {
        // Cloning must be done HERE since the plugin matcher will take ownership of candidate with `CString::from_raw`.
        // It leads to double free errors otherwise.
        let candidate = CString::new(s_path.clone()).ok()?.into_raw();
        if unsafe { (plugin.is_match)(candidate) } {
            let c_path = CString::new(path.display().to_string()).ok()?.into_raw();
            let output = unsafe { plugin.get_output(c_path) }.ok()?;
            return Some(PreviewBuilder::plugin_text(output, &plugin.name, path));
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
