use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use anyhow::Result;
use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use log::info;
use tuikit::prelude::{Attr, Color, Effect, Event, Term};

use crate::constant_strings_paths::NOTIFY_EXECUTABLE;
use crate::fileinfo::human_size;
use crate::opener::execute_in_child;

fn setup_progress_bar(
    action: String,
    size: (usize, usize),
) -> Result<(InMemoryTerm, ProgressBar, fs_extra::dir::CopyOptions)> {
    let (height, width) = size;
    let in_mem = InMemoryTerm::new(height as u16, width as u16);
    let pb = ProgressBar::with_draw_target(
        Some(100),
        ProgressDrawTarget::term_like(Box::new(in_mem.clone())),
    );
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner} {action} [{elapsed}] [{wide_bar}] {percent}% ({eta})",
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
        })
        .with_key("action", move |_: &ProgressState, w: &mut dyn Write| {
            write!(w, "{}", &action).unwrap()
        })
        .progress_chars("#>-"),
    );
    let options = fs_extra::dir::CopyOptions::new();
    Ok((in_mem, pb, options))
}

/// Display the updated progress bar on the terminal.
fn handle_progress_display(
    in_mem: &InMemoryTerm,
    pb: &ProgressBar,
    term: &Arc<Term>,
    process_info: fs_extra::TransitProcess,
) -> fs_extra::dir::TransitProcessResult {
    pb.set_position(progress_bar_position(&process_info));
    let _ = term.print_with_attr(1, 0, &in_mem.to_owned().contents(), CopyMove::attr());
    let _ = term.present();
    fs_extra::dir::TransitProcessResult::ContinueOrAbort
}

/// Position of the progress bar.
/// We have to handle properly 0 bytes to avoid division by zero.
fn progress_bar_position(process_info: &fs_extra::TransitProcess) -> u64 {
    if process_info.total_bytes > 0 {
        100 * process_info.copied_bytes / process_info.total_bytes
    } else {
        0
    }
}

/// Different kind of movement of files : copying or moving.
#[derive(Debug)]
pub enum CopyMove {
    Copy,
    Move,
}

impl CopyMove {
    fn attr() -> Attr {
        Attr {
            fg: Color::CYAN,
            bg: Color::Default,
            effect: Effect::REVERSE | Effect::BOLD,
        }
    }

    fn verb(&self) -> &str {
        match *self {
            CopyMove::Copy => "copy",
            CopyMove::Move => "move",
        }
    }

    fn preterit(&self) -> &str {
        match *self {
            CopyMove::Copy => "copied",
            CopyMove::Move => "moved",
        }
    }
}

/// Will copy or move a bunch of files to `dest`.
/// A progress bar is displayed on the passed terminal.
/// A notification is then sent to the user if a compatible notification system
/// is installed.
pub fn copy_move(
    copy_or_move: CopyMove,
    sources: Vec<PathBuf>,
    dest: &str,
    term: Arc<Term>,
) -> Result<()> {
    let c_term = term.clone();
    let (in_mem, pb, options) =
        setup_progress_bar(copy_or_move.verb().to_owned(), term.term_size()?)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&in_mem, &pb, &term, process_info)
    };
    let dest = dest.to_owned();
    let _ = thread::spawn(move || {
        let copier_mover = pick_copy_or_move(&copy_or_move);
        let transfered_bytes = match copier_mover(&sources, &dest, &options, handle_progress) {
            Ok(transfered_bytes) => transfered_bytes,
            Err(e) => {
                info!("copy move couldn't copy: {e:?}");
                0
            }
        };

        let _ = c_term.send_event(Event::User(()));

        inform_of_copy(
            copy_or_move.verb(),
            human_size(transfered_bytes),
            copy_or_move.preterit(),
        );
    });
    Ok(())
}

fn pick_copy_or_move<P, Q, F>(
    copy_or_move: &CopyMove,
) -> for<'a, 'b> fn(
    &'a [P],
    Q,
    &'b fs_extra::dir::CopyOptions,
    F,
) -> Result<u64, fs_extra::error::Error>
where
    P: AsRef<std::path::Path>,
    Q: AsRef<std::path::Path>,
    F: FnMut(fs_extra::TransitProcess) -> fs_extra::dir::TransitProcessResult,
{
    match copy_or_move {
        CopyMove::Copy => fs_extra::copy_items_with_progress,
        CopyMove::Move => fs_extra::move_items_with_progress,
    }
}

fn inform_of_copy(verb: &str, hs_bytes: String, preterit: &str) {
    let _ = notify(&format!("fm: {} finished {}B {}", verb, hs_bytes, preterit));
    info!("{} finished {}B", verb, hs_bytes,);
    info!(target: "special",
        "{} finished {}B",
        verb,
        hs_bytes,
    )
}

/// Send a notification to the desktop.
/// Requires "notify-send" to be installed.
fn notify(text: &str) -> Result<()> {
    execute_in_child(NOTIFY_EXECUTABLE, &[text])?;
    Ok(())
}
