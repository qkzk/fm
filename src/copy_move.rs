use std::sync::Arc;

use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget};
use std::path::PathBuf;
use tuikit::attr;
use tuikit::term::Term;

use crate::fm_error::FmResult;

pub struct CopierMover {
    term: Arc<Term>,
}

impl CopierMover {
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    fn setup(&self) -> FmResult<(InMemoryTerm, ProgressBar, fs_extra::dir::CopyOptions)> {
        let (height, width) = self.term.term_size()?;
        let in_mem = InMemoryTerm::new(height as u16, width as u16);
        let pb = ProgressBar::with_draw_target(
            None,
            ProgressDrawTarget::term_like(Box::new(in_mem.clone())),
        );
        let options = fs_extra::dir::CopyOptions::new(); //Initialize default values for CopyOptions
        Ok((in_mem, pb, options))
    }

    pub fn copy(&self, sources: Vec<&PathBuf>, dest: &str) -> FmResult<()> {
        let (in_mem, pb, options) = self.setup()?;
        let handle = |process_info: fs_extra::TransitProcess| {
            pb.set_position(100 * process_info.copied_bytes / process_info.total_bytes);
            let _ = self
                .term
                .print_with_attr(1, 0, &in_mem.contents(), attr::Attr::default());
            fs_extra::dir::TransitProcessResult::ContinueOrAbort
        };
        fs_extra::copy_items_with_progress(&sources, dest, &options, handle)?;
        pb.finish_with_message("done");
        Ok(())
    }

    pub fn mover(&self, sources: Vec<&PathBuf>, dest: &str) -> FmResult<()> {
        let (in_mem, pb, options) = self.setup()?;
        let handle = |process_info: fs_extra::TransitProcess| {
            pb.set_position(100 * process_info.copied_bytes / process_info.total_bytes);
            let _ = self
                .term
                .print_with_attr(1, 0, &in_mem.contents(), attr::Attr::default());
            fs_extra::dir::TransitProcessResult::ContinueOrAbort
        };
        fs_extra::move_items_with_progress(&sources, dest, &options, handle)?;
        pb.finish_with_message("done");
        Ok(())
    }
}
