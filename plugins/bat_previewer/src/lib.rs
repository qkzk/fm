use std::ffi::CString;
use std::os::raw::c_char;
use std::process::{Command, Stdio};

const NAME: &str = "bat previewer";
const EXE: &str = "bat";
const EXTENSIONS: &str = "rs py toml"

#[no_mangle]
pub extern "C" fn name() -> *mut c_char {
    CString::new(NAME)
        .expect("Couldn't create the name string")
        .into_raw()
}

#[no_mangle]
pub extern "C" fn extensions() -> *mut c_char {
    CString::new(EXTENSIONS)
        .expect("CString::new failed")
        .into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn preview(path: *mut c_char) -> *mut c_char {
    let output = unsafe {
        if !path.is_null() {
            let c_path = CString::from_raw(path);
            let r_path = c_path.into_string().expect("Into string failed");
            let output = Command::new(EXE)
                .arg(r_path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .expect("Command failed");
            if output.status.success() {
                String::from_utf8(output.stdout).expect("Wrong output")
            } else {
                String::from_utf8(output.stderr).expect("Wrong output")
            }
        } else {
            "path is empty".to_owned()
        }
    };
    CString::new(output)
        .expect("CString::new failed")
        .into_raw()
}
