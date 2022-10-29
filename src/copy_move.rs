use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget};
use tuikit::prelude::Term;

use crate::fm_error::FmResult;

fn setup(
    height: usize,
    width: usize,
) -> FmResult<(InMemoryTerm, ProgressBar, fs_extra::dir::CopyOptions)> {
    let in_mem = InMemoryTerm::new(height as u16, width as u16);
    let pb = ProgressBar::with_draw_target(
        Some(100),
        ProgressDrawTarget::term_like(Box::new(in_mem.clone())),
    );
    let options = fs_extra::dir::CopyOptions::new(); //Initialize default values for CopyOptions
    Ok((in_mem, pb, options))
}

fn handle_progress_display(
    in_mem: &InMemoryTerm,
    pb: &ProgressBar,
    term: &Arc<Term>,
    process_info: fs_extra::TransitProcess,
) -> fs_extra::dir::TransitProcessResult {
    pb.set_position(100 * process_info.copied_bytes / process_info.total_bytes);
    let _ = term.print(0, 0, &in_mem.to_owned().contents());
    let _ = term.present();
    fs_extra::dir::TransitProcessResult::ContinueOrAbort
}

pub fn copy(sources: Vec<PathBuf>, dest: String, term: Arc<Term>) -> FmResult<()> {
    let (height, width) = term.term_size()?;
    let (in_mem, pb, options) = setup(height, width)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&in_mem, &pb, &term, process_info)
    };
    let _ = thread::spawn(move || {
        fs_extra::copy_items_with_progress(&sources, &dest, &options, handle_progress)
            .unwrap_or_default();
    });
    Ok(())
}

pub fn mover(sources: Vec<PathBuf>, dest: String, term: Arc<Term>) -> FmResult<()> {
    let (height, width) = term.term_size()?;
    let (in_mem, pb, options) = setup(height, width)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&in_mem, &pb, &term, process_info)
    };
    let _ = thread::spawn(move || {
        fs_extra::move_items_with_progress(&sources, dest, &options, handle_progress)
            .unwrap_or_default();
    });
    Ok(())
}
