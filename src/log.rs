use anyhow::Result;
use log4rs;

use crate::constant_strings_paths::{ACTION_LOG_PATH, LOG_CONFIG_PATH};
use crate::utils::extract_lines;

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
