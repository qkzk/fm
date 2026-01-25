use std::fmt::{Display, Write};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

use anyhow::{bail, Context, Result};
use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};

use crate::common::{is_in_path, random_name, NOTIFY_EXECUTABLE};
use crate::event::FmEvents;
use crate::io::execute;
use crate::modes::human_size;
use crate::{log_info, log_line};

/// Store a copy or move.
/// It will be send as an Fm Event to update the marks.
#[derive(Default, Debug, Clone)]
pub struct DoneCopyMove {
    pub copy_move: CopyMove,
    pub from: PathBuf,
    pub final_to: PathBuf,
}

impl Display for DoneCopyMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{copy_move} from: {from} to: {to}",
            copy_move = self.copy_move.preterit(),
            from = self.from.display(),
            to = self.final_to.display()
        )
    }
}

// Won't replace with ratatui component since it's less flexible
/// Send the progress bar to event dispatcher, allowing its display
fn handle_progress_display(
    pb: &ProgressBar,
    process_info: fs_extra::TransitProcess,
) -> fs_extra::dir::TransitProcessResult {
    let progress = progress_bar_position(&process_info);
    pb.set_position(progress);
    fs_extra::dir::TransitProcessResult::ContinueOrAbort
}

/// Position of the progress bar.
/// We have to handle properly 0 bytes to avoid division by zero.
fn progress_bar_position(process_info: &fs_extra::TransitProcess) -> u64 {
    if process_info.total_bytes == 0 {
        return 0;
    }
    100 * process_info.copied_bytes / process_info.total_bytes
}

/// Different kind of movement of files : copying or moving.
#[derive(Default, Debug, Clone, Copy)]
pub enum CopyMove {
    Copy,
    #[default]
    Move,
}

impl CopyMove {
    /// True iff this operation is a copy and not a move.
    #[inline]
    pub fn is_copy(&self) -> bool {
        matches!(self, Self::Copy)
    }

    fn verb(&self) -> &str {
        match self {
            Self::Copy => "copy",
            Self::Move => "move",
        }
    }

    fn preterit(&self) -> &str {
        match self {
            Self::Copy => "copied",
            Self::Move => "moved",
        }
    }

    fn copier<P, Q, F>(
        &self,
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
        match self {
            Self::Copy => fs_extra::copy_items_with_progress,
            Self::Move => fs_extra::move_items_with_progress,
        }
    }

    fn log_and_notify(&self, hs_bytes: &str) {
        let message = format!("{preterit} {hs_bytes} bytes", preterit = self.preterit());
        let _ = notify(&message);
        log_info!("{message}");
        log_line!("{message}");
    }

    fn setup_progress_bar(
        &self,
        width: u16,
        height: u16,
    ) -> Result<(InMemoryTerm, ProgressBar, fs_extra::dir::CopyOptions)> {
        let width = width.saturating_sub(4);
        let in_mem = InMemoryTerm::new(height, width);
        let pb = ProgressBar::with_draw_target(
            Some(100),
            ProgressDrawTarget::term_like(Box::new(in_mem.clone())),
        );
        let action = self.verb().to_owned();
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner} {action} [{elapsed}] [{wide_bar}] {percent}% ({eta})",
            )?
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
}

/// Will copy or move a bunch of files to `dest`.
/// A progress bar is displayed.
/// A notification is then sent to the user if a compatible notification system
/// is installed.
///
/// If a file is copied or moved to a folder which already contains a file with the same name,
/// the copie/moved file has a `_` appended to its name.
///
/// This is done by :
/// 1. creating a random temporary folder in the destination,
/// 2. moving / copying every file there,
/// 3. moving all file to their final destination, appending enough `_` to get an unique file name,
/// 4. deleting the now empty temporary folder.
///
/// This quite complex behavior is the only way I could find to keep the progress bar while allowing to
/// create copies of files in the same dir.
///
/// It also sends an event "file copied" once all the files are copied
pub fn copy_move<P>(
    copy_move: CopyMove,
    sources: Vec<PathBuf>,
    dest: P,
    width: u16,
    height: u16,
    fm_sender: Arc<Sender<FmEvents>>,
) -> Result<InMemoryTerm>
where
    P: AsRef<std::path::Path>,
{
    let (in_mem, progress_bar, options) = copy_move.setup_progress_bar(width, height)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&progress_bar, process_info)
    };
    let mut conflict_handler = ConflictHandler::new(copy_move, &sources, dest)?;

    let _ = thread::spawn(move || {
        let transfered_bytes = match copy_move.copier()(
            &sources,
            &conflict_handler.temp_dest,
            &options,
            handle_progress,
        ) {
            Ok(transfered_bytes) => transfered_bytes,
            Err(e) => {
                log_info!("Error: {e:?}");
                log_line!("Error: {e:?}");
                0
            }
        };

        fm_sender.send(FmEvents::Refresh).unwrap_or_default();

        match conflict_handler.solve_conflicts() {
            Ok(done_copy_moves) => fm_sender
                .send(FmEvents::FileCopied(done_copy_moves))
                .unwrap_or_default(),
            Err(error) => log_info!("Conflict Handler error: {error}"),
        };

        copy_move.log_and_notify(&human_size(transfered_bytes));
    });
    Ok(in_mem)
}

/// Deal with conflicting filenames during a copy or a move.
struct ConflictHandler {
    copy_move: CopyMove,
    sources: Vec<PathBuf>,
    /// The destination of the files.
    /// If there's no conflicting filenames, it's their final destination
    /// otherwise it's a temporary folder we'll create.
    temp_dest: PathBuf,
    /// True iff there's at least one file name conflict:
    /// an already existing file in the destination with the same name
    /// as a file from source.
    has_conflict: bool,
    /// Defined to the final destination if there's a conflict.
    /// None otherwise.
    final_dest: Option<PathBuf>,
    done_copy_moves: Vec<DoneCopyMove>,
}

impl ConflictHandler {
    /// Creates a new `ConflictHandler` instance.
    /// We check for conflict and create the temporary folder if needed.
    fn new<P>(copy_move: CopyMove, sources: &[PathBuf], dest: P) -> Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        let has_conflict = ConflictHandler::check_filename_conflict(sources, &dest)?;
        let temp_dest: PathBuf;
        let final_dest: Option<PathBuf>;
        if has_conflict {
            temp_dest = Self::create_temporary_destination(&dest)?;
            final_dest = Some(dest.as_ref().to_path_buf());
        } else {
            temp_dest = dest.as_ref().to_path_buf();
            final_dest = None;
        };
        let done_copy_moves = vec![];
        let sources = sources.to_vec();

        Ok(Self {
            copy_move,
            sources,
            temp_dest,
            has_conflict,
            final_dest,
            done_copy_moves,
        })
    }

    /// Creates a randomly named folder in the destination.
    /// The name is `fm-random` where `random` is a random string of length 7.
    fn create_temporary_destination<P>(dest: P) -> Result<PathBuf>
    where
        P: AsRef<std::path::Path>,
    {
        let mut temp_dest = dest.as_ref().to_path_buf();
        let rand_str = random_name();
        temp_dest.push(rand_str);
        std::fs::create_dir(&temp_dest)?;
        Ok(temp_dest)
    }

    /// Move every file from `temp_dest` to `final_dest`.
    /// If the `final_dest` already contains a file with the same name,
    /// the moved file has enough `_` appended to its name to make it unique.
    fn move_copied_files_to_dest(&mut self) -> Result<()> {
        while let Some(from) = self.sources.pop() {
            let done_copy_move = self.move_single_file_to_dest(from)?;
            self.done_copy_moves.push(done_copy_move);
        }

        if self.temp_dest.read_dir()?.next().is_some() {
            bail!(
                "temp_dest {temp_dest} should be empty.",
                temp_dest = self.temp_dest.display()
            )
        }
        Ok(())
    }

    /// Move a single file to `final_dest`.
    /// If the file already exists in `final_dest` the moved one has enough '_' appended
    /// to its name to make it unique.
    fn move_single_file_to_dest(&mut self, from: PathBuf) -> Result<DoneCopyMove> {
        let filename = from.file_name().context("Should have a filename")?;
        let mut filename = filename.to_string_lossy().to_string();
        let mut temp_path = self.temp_dest.clone();
        temp_path.push(&filename);

        let mut final_to = self
            .final_dest
            .clone()
            .context("Final dest shouldn't be None")?;
        final_to.push(&filename);
        while final_to.exists() {
            final_to.pop();
            filename.push('_');
            final_to.push(&filename);
        }
        std::fs::rename(temp_path, &final_to)?;
        Ok(DoneCopyMove {
            copy_move: self.copy_move,
            from,
            final_to,
        })
    }

    /// True iff `dest` contains any file with the same file name as one of `sources`.
    fn check_filename_conflict<P>(sources: &[PathBuf], dest: P) -> Result<bool>
    where
        P: AsRef<std::path::Path>,
    {
        for file in sources {
            let filename = file.file_name().context("Couldn't read filename")?;
            let mut new_path = dest.as_ref().to_path_buf();
            new_path.push(filename);
            if new_path.exists() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Does nothing if there's no conflicting filenames during the copy/move.
    /// Move back every file, appending '_' to their name until the name is unique.
    /// Delete the temp folder.
    fn solve_conflicts(&mut self) -> Result<Vec<DoneCopyMove>> {
        if self.has_conflict {
            self.move_copied_files_to_dest()?;
            self.delete_temp_dest()?;
        } else {
            self.build_non_conflict_copy_moves()?;
        }
        Ok(std::mem::take(&mut self.done_copy_moves))
    }

    fn build_non_conflict_copy_moves(&mut self) -> Result<()> {
        while let Some(from) = self.sources.pop() {
            let filename = from.file_name().context("Should have a filename")?;

            let mut final_to = self.temp_dest.clone();
            final_to.push(filename);
            self.done_copy_moves.push(DoneCopyMove {
                copy_move: self.copy_move,
                from,
                final_to,
            })
        }
        Ok(())
    }

    /// Delete the temporary folder used when copying files.
    /// An error is returned if the temporary foldern isn't empty which
    /// should always be the case.
    fn delete_temp_dest(&self) -> Result<()> {
        std::fs::remove_dir(&self.temp_dest)?;
        Ok(())
    }
}

impl Drop for ConflictHandler {
    fn drop(&mut self) {
        let _ = self.delete_temp_dest();
    }
}

/// Send a notification to the desktop.
/// Does nothing if "notify-send" isn't installed.
fn notify(text: &str) -> Result<()> {
    if is_in_path(NOTIFY_EXECUTABLE) {
        execute(NOTIFY_EXECUTABLE, &[text])?;
    }
    Ok(())
}
