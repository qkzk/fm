use std::borrow::Borrow;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct FmError {
    pub details: String,
}

impl FmError {
    pub fn new(msg: &str) -> Self {
        Self {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for FmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for FmError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl From<std::io::Error> for FmError {
    fn from(err: std::io::Error) -> Self {
        Self::new(&err.to_string())
    }
}

impl From<regex::Error> for FmError {
    fn from(err: regex::Error) -> Self {
        Self::new(&err.to_string())
    }
}

impl From<std::ffi::OsString> for FmError {
    fn from(os_string: std::ffi::OsString) -> Self {
        Self::new(os_string.to_string_lossy().borrow())
    }
}

pub type FmResult<T> = Result<T, FmError>;
