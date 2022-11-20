use std::borrow::Borrow;
use std::error::Error;
use std::fmt;

use fs_extra::error::Error as FsExtraError;
use log::SetLoggerError;
use tuikit::error::TuikitError;
use zip::result::ZipError;

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

impl From<Box<dyn std::any::Any + Send + 'static>> for FmError {
    fn from(thread_error: Box<dyn std::any::Any + Send + 'static>) -> Self {
        Self::new(
            thread_error
                .downcast_ref::<&str>()
                .unwrap_or(&"Unreadable error from thread"),
        )
    }
}

impl From<TuikitError> for FmError {
    fn from(tuikit_error: TuikitError) -> Self {
        Self::new(&tuikit_error.to_string())
    }
}

impl From<Box<dyn Error + Send + Sync + 'static>> for FmError {
    fn from(err: Box<dyn Error + Send + Sync + 'static>) -> Self {
        Self::new(&err.to_string())
    }
}

impl From<FsExtraError> for FmError {
    fn from(fs_extra_error: FsExtraError) -> Self {
        Self::new(&fs_extra_error.to_string())
    }
}

impl From<ZipError> for FmError {
    fn from(zip_error: ZipError) -> Self {
        Self::new(&zip_error.to_string())
    }
}

impl From<SetLoggerError> for FmError {
    fn from(error: SetLoggerError) -> Self {
        Self::new(&error.to_string())
    }
}

impl From<Box<dyn Error>> for FmError {
    fn from(error: Box<dyn Error>) -> Self {
        Self::new(&error.to_string())
    }
}

impl From<exif::Error> for FmError {
    fn from(error: exif::Error) -> Self {
        Self::new(&error.to_string())
    }
}

pub type FmResult<T> = Result<T, FmError>;
