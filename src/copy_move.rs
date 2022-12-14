use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use log::info;
use notify_rust::Notification;
use tuikit::prelude::{Attr, Color, Effect, Event, Term};

use crate::fileinfo::human_size;
use crate::fm_error::FmResult;

fn setup(
    action: String,
    height: usize,
    width: usize,
) -> FmResult<(InMemoryTerm, ProgressBar, fs_extra::dir::CopyOptions)> {
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

fn handle_progress_display(
    in_mem: &InMemoryTerm,
    pb: &ProgressBar,
    term: &Arc<Term>,
    process_info: fs_extra::TransitProcess,
) -> fs_extra::dir::TransitProcessResult {
    pb.set_position(100 * process_info.copied_bytes / process_info.total_bytes);
    let _ = term.print_with_attr(
        1,
        0,
        &in_mem.to_owned().contents(),
        Attr {
            fg: Color::CYAN,
            bg: Color::default(),
            effect: Effect::REVERSE | Effect::BOLD,
        },
    );
    let _ = term.present();
    fs_extra::dir::TransitProcessResult::ContinueOrAbort
}

/// Different kind of movement of files : copying or moving.
#[derive(Debug)]
pub enum CopyMove {
    Copy,
    Move,
}

impl CopyMove {
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
) -> FmResult<()> {
    let c_term = term.clone();
    let (height, width) = term.term_size()?;
    let (in_mem, pb, options) = setup(copy_or_move.verb().to_owned(), height, width)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&in_mem, &pb, &term, process_info)
    };
    let dest = dest.to_owned();
    let _ = thread::spawn(move || {
        let copier_mover = match copy_or_move {
            CopyMove::Copy => fs_extra::copy_items_with_progress,
            CopyMove::Move => fs_extra::move_items_with_progress,
        };
        let transfered_bytes = match copier_mover(&sources, &dest, &options, handle_progress) {
            Ok(transfered_bytes) => transfered_bytes,
            Err(e) => {
                info!("copy move couldn't copy: {:?}", e);
                0
            }
        };

        let _ = c_term.send_event(Event::User(()));
        let _ = notify(
            &format!("fm: {} finished", copy_or_move.verb()),
            &format!(
                "{}B {}",
                human_size(transfered_bytes),
                copy_or_move.preterit()
            ),
        );
        info!(
            "{} finished {}B",
            copy_or_move.verb(),
            human_size(transfered_bytes)
        )
    });
    Ok(())
}

/// Send a notification to the desktop.
pub fn notify(summary: &str, body: &str) -> FmResult<()> {
    Notification::new().summary(summary).body(body).show()?;
    Ok(())
}
