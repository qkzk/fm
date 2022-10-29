use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use tuikit::prelude::{Attr, Color, Effect, Event, Term};

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

pub fn copy(sources: Vec<PathBuf>, dest: String, term: Arc<Term>) -> FmResult<()> {
    let c_term = term.clone();
    let (height, width) = term.term_size()?;
    let (in_mem, pb, options) = setup("copy".to_owned(), height, width)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&in_mem, &pb, &term, process_info)
    };
    let _ = thread::spawn(move || {
        fs_extra::copy_items_with_progress(&sources, &dest, &options, handle_progress)
            .unwrap_or_default();
        let _ = c_term.send_event(Event::User(()));
    });
    Ok(())
}

pub fn mover(sources: Vec<PathBuf>, dest: String, term: Arc<Term>) -> FmResult<()> {
    let c_term = term.clone();
    let (height, width) = term.term_size()?;
    let (in_mem, pb, options) = setup("move".to_owned(), height, width)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&in_mem, &pb, &term, process_info)
    };
    let _ = thread::spawn(move || {
        fs_extra::move_items_with_progress(&sources, dest, &options, handle_progress)
            .unwrap_or_default();
        let _ = c_term.send_event(Event::User(()));
    });
    Ok(())
}
