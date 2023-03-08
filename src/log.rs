use log4rs;

use crate::constant_strings_paths::{ACTION_LOG_PATH, LOG_CONFIG_PATH};
use crate::fm_error::FmResult;
use crate::utils::extract_last_lines_poorly;

pub fn set_loggers() -> FmResult<()> {
    log4rs::init_file(
        shellexpand::tilde(LOG_CONFIG_PATH).as_ref(),
        Default::default(),
    )?;
    Ok(())
}

/// Returns the last line of the log file.
pub fn read_log(line_nr: usize) -> FmResult<Vec<String>> {
    let log_path = shellexpand::tilde(ACTION_LOG_PATH).to_string();
    let content = std::fs::read_to_string(log_path)?;
    Ok(extract_last_lines_poorly(content, line_nr))
}
