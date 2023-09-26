use std::sync::RwLock;

use anyhow::Result;
use chrono::{offset, LocalResult, NaiveDateTime, Utc};
use lazy_static::lazy_static;
use log4rs;

use crate::constant_strings_paths::{ACTION_LOG_PATH, LOG_CONFIG_PATH};
use crate::utils::extract_lines;

const ONE_DAY_IN_SECONDS: i64 = 24 * 60 * 60;
// static ENV_HOME: &str = "$ENV{HOME}";

/// Set the logs.
/// The configuration is read from a config file defined in `LOG_CONFIG_PATH`
/// It's a YAML file which defines 2 logs:
/// - a normal one used directly with the macros like `log::info!(...)`, used for debugging
/// - a special one used with `log::info!(target: "special", ...)` to be displayed in the application
pub fn set_loggers() -> Result<()> {
    log4rs::init_file(
        shellexpand::tilde(LOG_CONFIG_PATH).as_ref(),
        Default::default(),
    )?;
    // clear_useless_env_home()?;
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
    let log_path = shellexpand::tilde(ACTION_LOG_PATH).to_string();
    let content = std::fs::read_to_string(log_path)?;
    Ok(extract_lines(content))
}

/// Parse log line, reads its date and compare it to Utc now.
/// True, If the date is readable and less than a day ago.
pub fn is_log_recent_engough(line: &str) -> Result<bool> {
    let strings: Vec<&str> = line.split(" - ").collect();
    if strings.is_empty() {
        Ok(false)
    } else {
        Ok(is_recent_enough(to_naive_datetime(strings[0])?))
    }
}

/// Parse a string like "2023-09-19 20:44:22" to a `chrono::NaiveDateTime`.
fn to_naive_datetime(str_date: &str) -> Result<NaiveDateTime> {
    Ok(NaiveDateTime::parse_from_str(
        str_date,
        "%Y-%m-%d %H:%M:%S",
    )?)
}

/// True iff the datetime can be parsed to Utc and is less than a day ago.
fn is_recent_enough(dt_naive: NaiveDateTime) -> bool {
    if let LocalResult::Single(dt_utc) = dt_naive.and_local_timezone(offset::Local) {
        let delta = Utc::now().timestamp() - dt_utc.timestamp();
        delta < ONE_DAY_IN_SECONDS
    } else {
        false
    }
}

lazy_static! {
    static ref LAST_LOG_LINE: RwLock<String> = RwLock::new("".to_string());
}

/// Read the last value of the "log line".
/// It's a global string created with `lazy_static!(...)`
pub fn read_last_log_line() -> String {
    let Ok(r) = LAST_LOG_LINE.read() else {
        return "".to_owned();
    };
    r.to_string()
}

/// Write a new log line to the global variable `LAST_LOG_LINE`.
/// It uses `lazy_static` to manipulate the global variable.
/// Fail silently if the global variable can't be written.
fn write_last_log_line(log: &str) {
    let Ok(mut new_log_string) = LAST_LOG_LINE.write() else {
        return;
    };
    *new_log_string = log.to_owned();
}

/// Write a line to both the global variable `LAST_LOG_LINE` and the special log
/// which can be displayed with Alt+l
pub fn write_log_line(log_line: String) {
    log::info!(target: "special", "{log_line}");
    write_last_log_line(&log_line);
}
