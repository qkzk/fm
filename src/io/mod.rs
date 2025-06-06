//! Everything about IO -- except input from terminal.
//! It's responsible for the argument parsing, the display, the execution of commands, the logs, accessing the cloud (google drive only ATM) and opening files.
//!
//! - [`args::Args`] the argument parser from execution of fm,
//! - `commands` a bunch of public function for various execution of commands: do we need to specify some arguments ? Is it a sudo command ? Do we need its output ? Should it never fail etc. fm relies a lot on executing commands so there's always a new situation which require a few different parameters. All commands should be executed from here.
//! - [`display::Display`] the displayer itself. All terminal display is made there. It's a single file, since why not ? with a single entry point. It then displays one to four windows after splitting the screen. This struct changed a lot after migration from tuikit to ratatui and is subject to a lot of internal changement.
//! - [`draw_menu::DrawMenu`] is a trait used to display most of the menus. It's implemented directly most of the time.
//! - [`git::git`] & [`git::git_root`] are function related to.. git. They're used to display the git porcelain v2 infos at the bottom and move to the git root of current folder.
//! - [`input_history::InputHistory`] is a basic history of text inputs, filtered by menu mode. It's used to allow moving back to a previous input without remembering it. Don't forget that logs are disabled by default and require the argument flag `-l` to be enabled.
//! - `log` contains a few functions to setup, read & write to logs. They're used everywhere in the application for debugging (obviously) but also to display what the last action did.
//! - [`opendal::OpendalContainer`] is the central struct dealing the google drive files, once the connection is established.
//! - [`opener::Opener`] and other structs of this file are used to open files. The opener are configurable in the config files.

mod args;
mod commands;
mod display;
mod draw_menu;
mod git;
mod image_adapter;
mod input_history;
mod log;
mod opendal;
mod opener;
mod ueberzug;

pub use args::Args;
pub use commands::*;
pub use display::{color_to_style, Display, Offseted, MIN_WIDTH_FOR_DUAL_PANE};
pub use draw_menu::*;
pub use git::{git, git_root};
pub use image_adapter::*;
pub use input_history::*;
pub use log::{read_last_log_line, read_log, set_loggers, write_log_info_once, write_log_line};
pub use opendal::*;
pub use opener::*;
pub use ueberzug::*;
