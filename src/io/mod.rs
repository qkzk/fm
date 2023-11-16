mod args;
mod git;
mod log;
mod opener;
mod term_manager;

pub use args::Args;
pub use git::{git, git_root};
pub use log::{read_last_log_line, read_log, set_loggers, write_log_line};
pub use opener::*;
pub use term_manager::{Display, EventReader, MIN_WIDTH_FOR_DUAL_PANE};
