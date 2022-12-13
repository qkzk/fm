use std::borrow::Borrow;
use std::error::Error;
use std::fmt;

use fs_extra::error::Error as FsExtraError;
use log::{info, SetLoggerError};
use notify_rust::error::Error as NotifyError;
use strfmt::FmtError;
use tuikit::error::TuikitError;
use zip::result::ZipError;

#[derive(Debug)]
pub enum ErrorVariant {
    IO,
    REGEX,
    OSSTRING,
    THREAD,
    TUIKIT,
    BOXED,
    FSEXTRA,
    ZIP,
    LOGGER,
    EXIF,
    NOTIFY,
    FMT,
    STRUM,
    COMPRESSTOOLS,
    CUSTOM(String),
}

#[derive(Debug)]
pub struct FmError {
    variant: ErrorVariant,
    pub details: String,
}

impl FmError {
    pub fn new(variant: ErrorVariant, msg: &str) -> Self {
        info!("FmError. Variant: {:?} - msg: {}", variant, msg);
        Self {
            variant,
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for FmError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} - {}", self.variant, self.details)
    }
}

impl Error for FmError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl From<std::io::Error> for FmError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorVariant::IO, &err.to_string())
    }
}

impl From<regex::Error> for FmError {
    fn from(err: regex::Error) -> Self {
        Self::new(ErrorVariant::REGEX, &err.to_string())
    }
}

impl From<std::ffi::OsString> for FmError {
    fn from(os_string: std::ffi::OsString) -> Self {
        Self::new(ErrorVariant::OSSTRING, os_string.to_string_lossy().borrow())
    }
}

impl From<Box<dyn std::any::Any + Send + 'static>> for FmError {
    fn from(thread_error: Box<dyn std::any::Any + Send + 'static>) -> Self {
        Self::new(
            ErrorVariant::THREAD,
            thread_error
                .downcast_ref::<&str>()
                .unwrap_or(&"Unreadable error from thread"),
        )
    }
}

impl From<TuikitError> for FmError {
    fn from(tuikit_error: TuikitError) -> Self {
        Self::new(ErrorVariant::TUIKIT, &tuikit_error.to_string())
    }
}

impl From<Box<dyn Error + Send + Sync + 'static>> for FmError {
    fn from(err: Box<dyn Error + Send + Sync + 'static>) -> Self {
        Self::new(ErrorVariant::BOXED, &err.to_string())
    }
}

impl From<FsExtraError> for FmError {
    fn from(fs_extra_error: FsExtraError) -> Self {
        Self::new(ErrorVariant::FSEXTRA, &fs_extra_error.to_string())
    }
}

impl From<ZipError> for FmError {
    fn from(zip_error: ZipError) -> Self {
        Self::new(ErrorVariant::ZIP, &zip_error.to_string())
    }
}

impl From<SetLoggerError> for FmError {
    fn from(error: SetLoggerError) -> Self {
        Self::new(ErrorVariant::LOGGER, &error.to_string())
    }
}

impl From<Box<dyn Error>> for FmError {
    fn from(error: Box<dyn Error>) -> Self {
        Self::new(ErrorVariant::BOXED, &error.to_string())
    }
}

impl From<exif::Error> for FmError {
    fn from(error: exif::Error) -> Self {
        Self::new(ErrorVariant::EXIF, &error.to_string())
    }
}

impl From<NotifyError> for FmError {
    fn from(error: NotifyError) -> Self {
        Self::new(ErrorVariant::NOTIFY, &error.to_string())
    }
}

impl From<FmtError> for FmError {
    fn from(error: FmtError) -> Self {
        Self::new(ErrorVariant::FMT, &error.to_string())
    }
}

impl From<strum::ParseError> for FmError {
    fn from(error: strum::ParseError) -> Self {
        Self::new(ErrorVariant::STRUM, &error.to_string())
    }
}

impl From<compress_tools::Error> for FmError {
    fn from(error: compress_tools::Error) -> Self {
        Self::new(ErrorVariant::COMPRESSTOOLS, &error.to_string())
    }
}

pub type FmResult<T> = Result<T, FmError>;
