use log::LevelFilter;
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

use crate::fm_error::FmResult;

static LOG_PATH: &str = "~/.config/fm/fm.log";

pub fn set_logger() -> FmResult<Handle> {
    let window_size = 3; // log0, log1, log2
    let fixed_window_roller = FixedWindowRoller::builder()
        .build("log{}", window_size)
        .unwrap();
    let size_limit = 5 * 1024; // 5KB as max log file size to roll
    let size_trigger = SizeTrigger::new(size_limit);
    let compound_policy =
        CompoundPolicy::new(Box::new(size_trigger), Box::new(fixed_window_roller));
    let log_path = shellexpand::tilde(LOG_PATH).to_string();
    // Don't propagate the error with ? since it crashes the application.
    let _ = std::fs::create_dir_all(&log_path);
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
                            .encoder(Box::new(PatternEncoder::new("{d} {l}::{m}{n}")))
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