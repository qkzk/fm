use std::ffi::CString;
use std::io::prelude::*;
use std::os::raw::c_char;
use std::process::{Command, Stdio};

const NAME: &str = "bat previewer";
const EXE: &str = "bat";
const EXTENSIONS: &str = "rs md toml";

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
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open("/home/quentin/gclem/dev/rust/fm/plugins/bat_previewer/bat_preview.log")
        .unwrap();
    let output = unsafe {
        if !path.is_null() {
            let c_path = CString::from_raw(path);
            let r_path = c_path.into_string().expect("Into string failed");
            writeln!(file, "{r_path}").expect("Couldn't write to file");
            run_bat(r_path)
        } else {
            "path is empty".to_owned()
        }
    };

    writeln!(file, "{output:.60}").expect("Couldn't write to file");
    CString::new(output)
        .expect("CString::new failed")
        .into_raw()
}

fn run_bat(r_path: String) -> String {
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
}
