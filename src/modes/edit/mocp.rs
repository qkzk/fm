use anyhow::Result;

use crate::app::Status;
use crate::app::Tab;
use crate::common::OPENER_AUDIO;
use crate::io::{execute, execute_and_capture_output, execute_and_capture_output_without_check};
use crate::log_info;

pub const MOCP: &str = OPENER_AUDIO.0;

/// A bunch of methods to control MOC.
/// It relies on the application `mocp` itself to :
/// - start & server (if needed) and add a song/folder to the current playlist,
/// - toggle pause / play,
/// - go to next song,
/// - go to previous song,
///
/// It should never fail but may force a refresh and flicker the screen if the server
/// wasn't running already.
pub struct Mocp {}

impl Mocp {
    /// Add a song or a folder to MOC playlist. Start it first...
    ///
    /// # Errors
    ///
    /// It should never fail.
    pub fn add_to_playlist(tab: &Tab) -> Result<()> {
        let _ = execute_and_capture_output_without_check(MOCP, &["-S"]);
        let Some(path_str) = tab.path_content.selected_path_string() else {
            return Ok(());
        };
        log_info!("mocp add to playlist {path_str:?}");
        let _ = execute_and_capture_output_without_check(MOCP, &["-a", &path_str]);
        Ok(())
    }

    /// Move to the currently playing song.
    ///
    /// # Errors
    ///
    /// It may fail if the command `mocp -Q %file` fails
    pub fn go_to_song(tab: &mut Tab) -> Result<()> {
        let output = execute_and_capture_output_without_check(MOCP, &["-Q", "%file"])?;
        let filepath = std::path::PathBuf::from(output.trim());
        let Some(parent) = filepath.parent() else {
            return Ok(());
        };
        let Some(filename) = filepath.file_name() else {
            return Ok(());
        };
        let Some(filename) = filename.to_str() else {
            return Ok(());
        };
        tab.cd(parent)?;
        tab.search_from(filename, 0);
        Ok(())
    }

    /// Toggle play/pause on MOC.
    /// Starts the server if needed, preventing the output to fill the screen.
    /// Then toggle play/pause
    ///
    /// # Errors
    ///
    /// It should never fail
    pub fn toggle_pause(status: &mut Status) -> Result<()> {
        log_info!("mocp toggle pause");
        match execute_and_capture_output(MOCP, &["-i"]) {
            Ok(stdout) => {
                // server is runing
                if stdout.contains("STOP") {
                    // music is stopped, start playing music
                    let _ = execute_and_capture_output(MOCP, &["-p"]);
                } else {
                    // music is playing or paused, toggle play/pause
                    let _ = execute_and_capture_output(MOCP, &["-G"]);
                }
            }
            Err(e) => {
                status.force_clear();
                log_info!("mocp -i error:\n{e:?}");
                // server is stopped, start it.
                let c = execute(MOCP, &["-S"]);
                let Ok(mut c) = c else {
                    // it shouldn't fail, something is wrong. It's better not to do anything.
                    return Ok(());
                };
                let _ = c.wait();
                // start playing music
                let _ = execute_and_capture_output(MOCP, &["-p"]);
            }
        }
        Ok(())
    }

    /// Skip to the next song in MOC
    ///
    /// # Errors
    ///
    /// It should never fail
    pub fn next() -> Result<()> {
        log_info!("mocp next");
        let _ = execute_and_capture_output_without_check(MOCP, &["-f"]);
        Ok(())
    }

    /// Go to the previous song in MOC
    ///
    /// # Errors
    ///
    /// It should never fail
    pub fn previous() -> Result<()> {
        log_info!("mocp previous");
        let _ = execute_and_capture_output_without_check(MOCP, &["-r"]);
        Ok(())
    }

    /// Clear the playlist
    /// Since clearing the playlist exit the server,
    /// we have to restart it afterwards.
    ///
    /// # Errors
    ///
    /// It should never fail
    pub fn clear() -> Result<()> {
        log_info!("mocp clear");
        // Clear the playlist **and exit**
        let _ = execute_and_capture_output_without_check(MOCP, &["-c"]);
        // Restart the server
        let _ = execute_and_capture_output_without_check(MOCP, &["-S"]);
        Ok(())
    }
}
