use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use anyhow::{Context, Result};
use fs_extra;
use indicatif::{InMemoryTerm, ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
use log::info;
use tuikit::prelude::{Attr, Color, Effect, Event, Term};

use crate::constant_strings_paths::NOTIFY_EXECUTABLE;
use crate::fileinfo::human_size;
use crate::log::write_log_line;
use crate::opener::execute_in_child;
use crate::utils::{is_program_in_path, random_name};

/// Display the updated progress bar on the terminal.
fn handle_progress_display(
    in_mem: &InMemoryTerm,
    pb: &ProgressBar,
    term: &Arc<Term>,
    process_info: fs_extra::TransitProcess,
) -> fs_extra::dir::TransitProcessResult {
    pb.set_position(progress_bar_position(&process_info));
    let _ = term.print_with_attr(1, 1, &in_mem.contents(), CopyMove::attr());
    let _ = term.present();
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
    fn attr() -> Attr {
        Attr {
            fg: Color::CYAN,
            bg: Color::Default,
            effect: Effect::REVERSE | Effect::BOLD,
        }
    }

    fn verb(&self) -> &str {
        match self {
            CopyMove::Copy => "copy",
            CopyMove::Move => "move",
        }
    }

    fn preterit(&self) -> &str {
        match self {
            CopyMove::Copy => "copied",
            CopyMove::Move => "moved",
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
            CopyMove::Copy => fs_extra::copy_items_with_progress,
            CopyMove::Move => fs_extra::move_items_with_progress,
        }
    }

    fn log_and_notify(&self, hs_bytes: String) {
        let message = format!("{preterit} {hs_bytes} bytes", preterit = self.preterit());
        let _ = notify(&message);
        info!("{message}");
        write_log_line(message);
    }

    fn setup_progress_bar(
        &self,
        size: (usize, usize),
    ) -> Result<(InMemoryTerm, ProgressBar, fs_extra::dir::CopyOptions)> {
        let (height, width) = size;
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
/// A progress bar is displayed on the passed terminal.
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
/// In this scenario we have to wait for the copy to end before moving back the file.
/// Sadly, the copy is now blocking.
pub fn copy_move(
    copy_or_move: CopyMove,
    sources: Vec<PathBuf>,
    dest: &str,
    term: Arc<Term>,
) -> Result<()> {
    let c_term = term.clone();
    let (in_mem, pb, options) = copy_or_move.setup_progress_bar(term.term_size()?)?;
    let handle_progress = move |process_info: fs_extra::TransitProcess| {
        handle_progress_display(&in_mem, &pb, &term, process_info)
    };
    let dest_has_existing_file = check_existing_file(&sources, dest)?;
    let final_dest = dest;
    let dest = if dest_has_existing_file {
        create_temporary_destination(dest)?
    } else {
        dest.to_owned()
    };
    let temp_dest = dest.clone();
    let copy_move_thread = thread::spawn(move || {
        let transfered_bytes =
            match copy_or_move.copier()(&sources, &dest, &options, handle_progress) {
                Ok(transfered_bytes) => transfered_bytes,
                Err(e) => {
                    info!("copy move couldn't copy: {e:?}");
                    0
                }
            };

        let _ = c_term.send_event(Event::User(()));

        copy_or_move.log_and_notify(human_size(transfered_bytes));
    });
    if dest_has_existing_file {
        let _ = copy_move_thread.join();
        move_copied_files_to_dest(&temp_dest, final_dest)?;
    }
    Ok(())
}

/// Creates a randomly named folder in the destination.
/// The name is `fm-random` where `random` is a random string of length 7.
fn create_temporary_destination(dest: &str) -> Result<String> {
    let mut temp_dest = std::path::PathBuf::from(dest);
    let rand_str = random_name();
    temp_dest.push(rand_str);
    std::fs::create_dir(&temp_dest)?;
    Ok(temp_dest.display().to_string())
}

/// Move every file from `temp_dest` to `final_dest` and delete `temp_dest`.
/// If the `final_dest` already contains a file with the same name,
/// the moved file has enough `_` appended to its name to make it unique.
/// The now empty `temp_dest` is then deleted.
fn move_copied_files_to_dest(temp_dest: &str, final_dest: &str) -> Result<()> {
    for file in std::fs::read_dir(temp_dest).context("Unreachable folder")? {
        let file = file.context("File don't exist")?;
        move_copied_file_to_dest(file, final_dest)?;
    }

    std::fs::remove_dir(temp_dest)?;

    Ok(())
}

/// Move a single file to `final_dest`.
/// If the file already exists in `final_dest` the moved one has engough '_' appended
/// to its name to make it unique.
fn move_copied_file_to_dest(file: std::fs::DirEntry, final_dest: &str) -> Result<()> {
    let mut file_name = file
        .file_name()
        .to_str()
        .context("Couldn't cast the filename")?
        .to_owned();
    let mut old_dest = std::path::PathBuf::from(final_dest);
    old_dest.push(&file_name);
    while old_dest.exists() {
        old_dest.pop();
        file_name.push('_');
        old_dest.push(&file_name);
    }
    std::fs::rename(file.path(), old_dest)?;
    Ok(())
}

/// True iff `dest` contains any file with the same file name as one of `sources`.
fn check_existing_file(sources: &[PathBuf], dest: &str) -> Result<bool> {
    for file in sources {
        let filename = file
            .file_name()
            .context("Couldn't read filename")?
            .to_str()
            .context("Couldn't cast filename into str")?
            .to_owned();
        let mut new_path = std::path::PathBuf::from(dest);
        new_path.push(&filename);
        if new_path.exists() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Send a notification to the desktop.
/// Does nothing if "notify-send" isn't installed.
fn notify(text: &str) -> Result<()> {
    if is_program_in_path(NOTIFY_EXECUTABLE) {
        execute_in_child(NOTIFY_EXECUTABLE, &[text])?;
    }
    Ok(())
}
