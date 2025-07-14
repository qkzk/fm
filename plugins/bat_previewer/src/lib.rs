use std::ffi::CString;
use std::os::raw::c_char;
use std::process::{Command, Stdio};

const NAME: &str = "bat previewer";
const EXE: &str = "bat";
const EXTENSIONS: &str = "rs md toml";

/// Returns the name of the plugin.
///
/// This function is mandatory and should have exactly the same signature.
#[no_mangle]
pub extern "C" fn name() -> *mut c_char {
    CString::new(NAME)
        .expect("Couldn't create the name string")
        .into_raw()
}

// TODO: read a full path instead of the extension. Other condition may exist.

/// True if the extension can be previewed with this plugin.
///
/// This function is mandatory and should have exactly the same signature.
///
/// # Safety
///
/// this function is unsafe and should only be called from the host itself.
/// the string length must match what was send exactly.
#[no_mangle]
pub unsafe extern "C" fn is_match(ext_candidate: *mut c_char) -> bool {
    if !ext_candidate.is_null() {
        let Ok(candidate) = unsafe { CString::from_raw(ext_candidate) }.into_string() else {
            return false;
        };
        for ext in EXTENSIONS.split_whitespace() {
            if ext == candidate.to_lowercase() {
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

/// Execute `bat --color=always --style=numbers --theme=Dracula <path>` and returns its output.
fn run_bat(r_path: String) -> Result<String, std::io::Error> {
    let output = Command::new(EXE)
        .arg(r_path)
        .arg("--color=always")
        .arg("--style=numbers")
        .arg("--theme=Dracula")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    } else {
        String::from_utf8(output.stderr)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
    }
}
