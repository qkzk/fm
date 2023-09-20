use anyhow::Result;
use chrono::{offset, LocalResult, NaiveDateTime, Utc};
use log4rs;

use crate::constant_strings_paths::{ACTION_LOG_PATH, LOG_CONFIG_PATH};
use crate::utils::extract_lines;

const ONE_DAY_IN_SECONDS: i64 = 24 * 60 * 60;

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
    Ok(())
}

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
