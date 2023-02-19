use std::borrow::Borrow;
use std::error::Error;
use std::fmt;

use fs_extra::error::Error as FsExtraError;
use log::SetLoggerError;
use strfmt::FmtError;
use tuikit::error::TuikitError;

/// Different variant of errors, depending on what caused the error.
/// If the error is custom made, a string depicts the problem more precisely.
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
    FMT,
    STRUM,
    SERDEYAML,
    CHRONO,
    UTF8ERROR,
    ZIPERROR,
    LZMAERROR,
    CUSTOM(String),
}

/// Default error used in whole application.
#[derive(Debug)]
pub struct FmError {
    variant: ErrorVariant,
    details: String,
}

impl FmError {
    /// Creates a new `FmError` with a variant and a message.
    pub fn new(variant: ErrorVariant, msg: &str) -> Self {
        Self {
            variant,
            details: msg.to_string(),
        }
    }

    /// Creates a new CUSTOM error.
    /// Syntactic sugar
    pub fn custom(variant_str: &str, msg: &str) -> Self {
        Self::new(ErrorVariant::CUSTOM(variant_str.to_owned()), msg)
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

impl From<serde_yaml::Error> for FmError {
    fn from(error: serde_yaml::Error) -> Self {
        Self::new(ErrorVariant::SERDEYAML, &error.to_string())
    }
}

impl From<&std::io::Error> for FmError {
    fn from(error: &std::io::Error) -> Self {
        Self::new(ErrorVariant::IO, &error.to_string())
    }
}

impl From<chrono::ParseError> for FmError {
    fn from(error: chrono::ParseError) -> Self {
        Self::new(ErrorVariant::CHRONO, &error.to_string())
    }
}

impl From<std::string::FromUtf8Error> for FmError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Self::new(ErrorVariant::UTF8ERROR, &error.to_string())
    }
}

impl From<zip::result::ZipError> for FmError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::new(ErrorVariant::ZIPERROR, &error.to_string())
    }
}

impl From<lzma::LzmaError> for FmError {
    fn from(error: lzma::LzmaError) -> Self {
        Self::new(ErrorVariant::LZMAERROR, &error.to_string())
    }
}

/// A Result with type `T` and `FmError`.
pub type FmResult<T> = Result<T, FmError>;
