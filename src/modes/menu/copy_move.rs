use std::fmt::Write;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};

use crate::common::{is_in_path, random_name, NOTIFY_EXECUTABLE};
use crate::event::FmEvents;
use crate::io::execute;
use crate::modes::human_size;
use crate::{log_info, log_line};

// TODO replace with ratatui component
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
#[derive(Debug)]
pub enum CopyMove {
    Copy,
    Move,
}

impl CopyMove {
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
        width: usize,
        height: usize,
    ) -> Result<(InMemoryTerm, ProgressBar, fs_extra::dir::CopyOptions)> {
        let in_mem = InMemoryTerm::new(height as u16, width as u16);
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
    copy_or_move: CopyMove,
    sources: Vec<PathBuf>,
    dest: P,
    width: usize,
    height: usize,
    fm_sender: Arc<Sender<FmEvents>>,
) -> Result<InMemoryTerm>
where
    P: AsRef<std::path::Path>,
{
    let (in_mem, progress_bar, options) = copy_or_move.setup_progress_bar(width, height)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&progress_bar, process_info)
    };
    let conflict_handler = ConflictHandler::new(dest, &sources)?;

    let _ = thread::spawn(move || {
        let transfered_bytes = match copy_or_move.copier()(
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

        if let Err(e) = conflict_handler.solve_conflicts() {
            log_info!("Conflict Handler error: {e}");
        }

        copy_or_move.log_and_notify(&human_size(transfered_bytes));
        if matches!(copy_or_move, CopyMove::Copy) {
            fm_sender.send(FmEvents::FileCopied).unwrap_or_default();
        }
    });
    Ok(in_mem)
}

/// Deal with conflicting filenames during a copy or a move.
struct ConflictHandler {
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
}

impl ConflictHandler {
    /// Creates a new `ConflictHandler` instance.
    /// We check for conflict and create the temporary folder if needed.
    fn new<P>(dest: P, sources: &[PathBuf]) -> Result<Self>
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

        Ok(Self {
            temp_dest,
            has_conflict,
            final_dest,
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

    /// Move every file from `temp_dest` to `final_dest` and delete `temp_dest`.
    /// If the `final_dest` already contains a file with the same name,
    /// the moved file has enough `_` appended to its name to make it unique.
    fn move_copied_files_to_dest(&self) -> Result<()> {
        for file in std::fs::read_dir(&self.temp_dest).context("Unreachable folder")? {
            let file = file.context("File don't exist")?;
            self.move_single_file_to_dest(file)?;
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

    /// Move a single file to `final_dest`.
    /// If the file already exists in `final_dest` the moved one has enough '_' appended
    /// to its name to make it unique.
    fn move_single_file_to_dest(&self, file: std::fs::DirEntry) -> Result<()> {
        let mut file_name = file
            .file_name()
            .to_str()
            .context("Couldn't cast the filename")?
            .to_owned();

        let mut final_dest = self
            .final_dest
            .clone()
            .context("Final dest shouldn't be None")?;
        final_dest.push(&file_name);
        while final_dest.exists() {
            final_dest.pop();
            file_name.push('_');
            final_dest.push(&file_name);
        }
        std::fs::rename(file.path(), final_dest)?;
        Ok(())
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
    fn solve_conflicts(&self) -> Result<()> {
        if self.has_conflict {
            self.move_copied_files_to_dest()?;
            self.delete_temp_dest()?;
        }
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
