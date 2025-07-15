use std::ffi::CString;
use std::io;
use std::os::raw::c_char;
use std::path::Path;
use std::process::{Command, Stdio};

mod extensions;

use extensions::EXTENSIONS;

const NAME: &str = "bat previewer";
const EXE: &str = "bat";

/// Returns the name of the plugin.
///
/// This function is mandatory and should have exactly the same signature.
#[no_mangle]
pub extern "C" fn name() -> *mut c_char {
    CString::new(NAME)
        .expect("Couldn't create the name string")
        .into_raw()
}

/// True if the path can be previewed with this plugin.
///
/// This function is mandatory and should have exactly the same signature.
///
/// # Safety
///
/// this function is unsafe and should only be called from the host itself.
/// the string length must match what was send exactly.
#[no_mangle]
pub unsafe extern "C" fn is_match(c_path_candidate: *mut c_char) -> bool {
    if !c_path_candidate.is_null() {
        let Ok(s_candidate) = unsafe { CString::from_raw(c_path_candidate) }.into_string() else {
            return false;
        };
        let Some(ext_candidate) = Path::new(&s_candidate).extension() else {
            return false;
        };
        let ext_candidate = ext_candidate.to_string_lossy().to_string().to_lowercase();

        for ext in &EXTENSIONS {
            if *ext == ext_candidate {
                return true;
            }
        }
    }
    false
}

/// Returns the preview as a string (of many lines, with or without ansi escape bytes)
/// The path is an utf-8 valid path.
///
/// This function is mandatory and should have exactly the same signature.
///
/// # Safety
///
/// this function is unsafe and should only be called from the host itself.
/// the string length must match what was send exactly.
#[no_mangle]
pub unsafe extern "C" fn preview(path: *mut c_char) -> *mut c_char {
    let output = if !path.is_null() {
        if let Ok(r_path) = unsafe { CString::from_raw(path) }.into_string() {
            match run_bat(r_path) {
                Ok(output) => output,
                Err(e) => e.to_string(),
            }
        } else {
            "path contains invalid utf-8 bytes".to_owned()
        }
    } else {
        "path is empty".to_owned()
    };
    CString::new(output)
        .expect("CString::new failed")
        .into_raw()
}

/// Executes `bat --color=always --style=numbers --theme=Dracula <path>` and returns its output.
fn run_bat(r_path: String) -> Result<String, io::Error> {
    let output = Command::new(EXE)
        .arg(r_path)
        .arg("--color=always")
        .arg("--style=numbers")
        .arg("--theme=Dracula")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    let content = if output.status.success() {
        output.stdout
    } else {
        output.stderr
    };
    String::from_utf8(content).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}
