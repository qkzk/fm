use anyhow::Result;
use clap::Parser;
use log4rs;

use crate::common::extract_lines;
use crate::common::{tilde, ACTION_LOG_PATH, LOG_CONFIG_PATH};
use crate::config::{LAST_LOG_INFO, LAST_LOG_LINE};
use crate::io::Args;

/// Set the logs.
/// First we read the `-l` `--log` command line argument which default to false.
///
/// If it's false, nothing is done and we return.
/// No logger is set, nothing is logged.
///
/// If it's true :
/// The configuration is read from a config file defined in `LOG_CONFIG_PATH`
/// It's a YAML file which defines 2 logs:
/// - a normal one used directly with the macros like `log::info!(...)`, used for debugging
/// - a special one used with `log::info!(target: "special", ...)` to be displayed in the application
pub fn set_loggers() -> Result<()> {
    let args = Args::parse();
    if args.log {
        log4rs::init_file(tilde(LOG_CONFIG_PATH).as_ref(), Default::default())?;
        // clear_useless_env_home()?;

        log::info!("fm is starting with logs enabled");
    }
    Ok(())
}

/// Delete useless $ENV{HOME} folder created by log4rs.
/// This folder is created when a log file is big enough to proc a rolling
/// Since the pattern can't be resolved, it's not created in the config folder but where the app is started...
/// See [github issue](https://github.com/estk/log4rs/issues/314)
/// The function log its results and delete nothing.
// fn clear_useless_env_home() -> Result<()> {
//     let p = std::path::Path::new(&ENV_HOME);
//     let cwd = std::env::current_dir();
//     log::info!(
//         "looking from {ENV_HOME} - {p}  CWD {cwd}",
//         p = p.display(),
//         cwd = cwd?.display()
//     );
//     if p.exists() && std::fs::metadata(ENV_HOME)?.is_dir()
//     // && std::path::Path::new(ENV_HOME).read_dir()?.next().is_none()
//     {
//         let z = std::path::Path::new(ENV_HOME).read_dir()?.next();
//         log::info!("z {z:?}");
//
//         // std::fs::remove_dir_all(ENV_HOME)?;
//         log::info!("Removed {ENV_HOME} empty directory from CWD");
//     }
//     Ok(())
// }

/// Returns the last line of the log file.
pub fn read_log() -> Result<Vec<String>> {
    let log_path = tilde(ACTION_LOG_PATH).to_string();
    let content = std::fs::read_to_string(log_path)?;
    Ok(extract_lines(content))
}

/// Read the last value of the "log line".
/// Fail silently if the global variable can't be read and returns an empty string.
pub fn read_last_log_line() -> String {
    let Some(last_log_line) = LAST_LOG_LINE.get() else {
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
    let Ok(()) = LAST_LOG_LINE.set(log.to_string()) else {
        return;
    };
}

/// Write a line to both the global variable `LAST_LOG_LINE` and the special log
/// which can be displayed with Alt+l
pub fn write_log_line<S>(log_line: S)
where
    S: Into<String> + std::fmt::Display,
{
    log::info!(target: "special", "{log_line}");
    write_last_log_line(log_line);
}

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
    let Some(last_log_line) = LAST_LOG_INFO.get() else {
        return "".to_owned();
    };
    last_log_line.to_owned()
}

/// Write a new log info to the global variable `LAST_LOG_INFO`.
/// Fail silently if the global variable can't be written.
fn write_last_log_info<S>(log: &S)
where
    S: Into<String> + std::fmt::Display,
{
    let Ok(()) = LAST_LOG_INFO.set(log.to_string()) else {
        return;
    };
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
