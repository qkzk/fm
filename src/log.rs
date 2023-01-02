use log::{info, LevelFilter};
use log4rs::{
    append::rolling_file::{
        policy::compound::{
            roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
        },
        RollingFileAppender,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::threshold::ThresholdFilter,
    Handle,
};

use crate::constant_strings_paths::LOG_PATH;
use crate::fm_error::{FmError, FmResult};

fn create_log_folder(log_path: &str) -> FmResult<String> {
    let path_buf = std::path::PathBuf::from(log_path);
    let parent = path_buf.parent().ok_or_else(|| {
        FmError::custom(
            "create log folder",
            &format!(
                "Couldn't create log folder. LOGPATH {} should have a parent",
                LOG_PATH
            ),
        )
    })?;
    std::fs::create_dir_all(parent)?;

    Ok(parent.to_string_lossy().to_string())
}

/// Creates a default logger with rotating file logs.
/// 3 files à 5KB each are maintened.
/// The log files are located in $HOME/username/.config/fm
pub fn set_logger() -> FmResult<Handle> {
    let log_path = shellexpand::tilde(LOG_PATH).to_string();
    info!("log path: {}", log_path);
    create_log_folder(&log_path)?;

    let window_size = 3; // log0, log1, log2
    let fixed_window_roller = FixedWindowRoller::builder()
        .build(&log_path, window_size)
        .unwrap();
    let size_limit = 5 * 1024 * 1024; // 5MB as max log file size to roll
    let size_trigger = SizeTrigger::new(size_limit);
    let compound_policy =
        CompoundPolicy::new(Box::new(size_trigger), Box::new(fixed_window_roller));
    // Don't propagate the error with ? since it crashes the application.
    // Log Trace level output to file where trace is the default level
    // and the programmatically specified level to stderr.
    let config = Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(LevelFilter::Debug)))
                .build(
                    &log_path,
                    Box::new(
                        RollingFileAppender::builder()
                            .encoder(Box::new(PatternEncoder::new("{d} {l}::{m} - {f}:{L}{n}")))
                            .build(&log_path, Box::new(compound_policy))?,
                    ),
                ),
        )
        .build(Root::builder().appender(&log_path).build(LevelFilter::Info))
        .unwrap();

    // Use this to change log levels at runtime.
    // This means you can change the default log level to trace
    // if you are trying to debug an issue and need more logs on then turn it off
    // once you are done.
    Ok(log4rs::init_config(config)?)
}
