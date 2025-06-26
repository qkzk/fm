use std::fs::{metadata, remove_file, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::Mutex;
use std::sync::RwLock;

use anyhow::Result;
use chrono::Local;
use log::{Level, LevelFilter, Metadata, Record, SetLoggerError};

use crate::common::{extract_lines, tilde, ACTION_LOG_PATH, NORMAL_LOG_PATH};

/// Holds the last action which is displayed to the user
static LAST_LOG_LINE: RwLock<String> = RwLock::new(String::new());

/// Holds the last line of the log
static LAST_LOG_INFO: RwLock<String> = RwLock::new(String::new());

const MAX_LOG_SIZE: u64 = 50_000;

/// Setup of 2 loggers
/// - a normal one used directly with the macros like `log::info!(...)`, used for debugging
/// - an action one used with `log::info!(target: "action", ...)` to be displayed in the application
pub struct FMLogger {
    normal_log: Mutex<BufWriter<std::fs::File>>,
    action_log: Mutex<BufWriter<std::fs::File>>,
}

impl Default for FMLogger {
    fn default() -> Self {
        let normal_file = open_or_rotate(tilde(NORMAL_LOG_PATH).as_ref(), MAX_LOG_SIZE);
        let action_file = open_or_rotate(tilde(ACTION_LOG_PATH).as_ref(), MAX_LOG_SIZE);
        let normal_log = Mutex::new(BufWriter::new(normal_file));
        let action_log = Mutex::new(BufWriter::new(action_file));
        Self {
            normal_log,
            action_log,
        }
    }
}

impl FMLogger {
    pub fn init(self) -> Result<(), SetLoggerError> {
        log::set_boxed_logger(Box::new(self))?;
        log::set_max_level(LevelFilter::Info);
        log::info!("fm is starting with logs enabled");
        Ok(())
    }

    fn write(&self, writer: &Mutex<BufWriter<File>>, record: &Record) {
        let mut writer = writer.lock().unwrap();
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(writer, "{timestamp} - {msg}", msg = record.args());
        let _ = writer.flush();
    }
}

impl log::Log for FMLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if record.target() == "action" {
            self.write(&self.action_log, record)
        } else {
            self.write(&self.normal_log, record)
        }
    }

    fn flush(&self) {
        let _ = self.normal_log.lock().unwrap().flush();
        let _ = self.action_log.lock().unwrap().flush();
    }
}

fn open_or_rotate(path: &str, max_size: u64) -> File {
    if let Ok(meta) = metadata(path) {
        if meta.len() > max_size {
            let _ = remove_file(path);
        }
    }

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("cannot open log file")
}

/// Returns the last line of the log file.
pub fn read_log() -> Result<Vec<String>> {
    let log_path = tilde(ACTION_LOG_PATH).to_string();
    let content = std::fs::read_to_string(log_path)?;
    Ok(extract_lines(content))
}

/// Read the last value of the "log line".
/// Fail silently if the global variable can't be read and returns an empty string.
pub fn read_last_log_line() -> String {
    let Ok(last_log_line) = LAST_LOG_LINE.read() else {
        return "".to_owned();
    };
    last_log_line.to_owned()
}

/// Write a new log line to the global variable `LAST_LOG_LINE`.
/// Fail silently if the global variable can't be written.
fn write_last_log_line<S>(log: S)
where
    S: Into<String> + std::fmt::Display,
{
    let Ok(mut last_log_line) = LAST_LOG_LINE.write() else {
        log::info!("Couldn't write to LAST_LOG_LINE");
        return;
    };
    *last_log_line = log.to_string();
}

/// Write a line to both the global variable `LAST_LOG_LINE` and the action log
/// which can be displayed with Alt+l
pub fn write_log_line<S>(log_line: S)
where
    S: Into<String> + std::fmt::Display,
{
    log::info!(target: "action", "{log_line}");
    write_last_log_line(log_line);
}

/// Writes the message to the global variable `LAST_LOG_LINE` and a the action log.
/// It can be displayed with the default bind ALt+l and at the last line of the display.
/// Every action which change the filetree, execute an external command or which returns an
/// error should be logged this way.
#[macro_export]
macro_rules! log_line {
    ($($arg:tt)+) => (
    $crate::io::write_log_line(
      format!($($arg)+)
    )
  );
}

/// Read the last value of the "log info".
/// Fail silently if the global variable can't be read and returns an empty string.
fn read_last_log_info() -> String {
    let Ok(last_log_info) = LAST_LOG_INFO.read() else {
        return "".to_owned();
    };
    last_log_info.to_owned()
}

/// Write a new log info to the global variable `LAST_LOG_INFO`.
/// Fail silently if the global variable can't be written.
fn write_last_log_info<S>(log: &S)
where
    S: Into<String> + std::fmt::Display,
{
    let Ok(mut last_log_info) = LAST_LOG_INFO.write() else {
        log::info!("Couldn't write to LAST_LOG_LINE");
        return;
    };
    *last_log_info = log.to_string();
}

/// Write a line to both the global variable `LAST_LOG_INFO` and the info log
/// Won't write the same line multiple times during the same execution.
pub fn write_log_info_once<S>(log_line: S)
where
    S: Into<String> + std::fmt::Display,
{
    if read_last_log_info() != log_line.to_string() {
        write_last_log_info(&log_line);
        log::info!("{log_line}");
    }
}

/// Log a formated message to the default log.
/// Won't write anything if the same message is sent multiple times.
/// It uses `log::info!` internally.
/// It accepts the same formatted messages as `format`.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)+) => (
    $crate::io::write_log_info_once(
      format!($($arg)+)
    )
  );
}
