use anyhow::Result;
use log::info;

use crate::opener::{execute_and_capture_output, execute_in_child};
use crate::status::Status;
use crate::tab::Tab;

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
    pub fn add_to_playlist(tab: &Tab) -> Result<()> {
        let _ = execute_in_child("mocp", &vec!["-S"]);
        let Some(path_str) = tab.path_content.selected_path_string() else { return Ok(()); };
        info!("mocp add to playlist {path_str:?}");
        let _ = execute_in_child("mocp", &vec!["-a", &path_str]);
        Ok(())
    }

    /// Toggle play/pause on MOC.
    /// Starts the server if needed, preventing the output to fill the screen.
    /// Then toggle play/pause
    pub fn toggle_pause(status: &mut Status) -> Result<()> {
        info!("mocp toggle pause");
        match execute_and_capture_output("mocp", &vec!["-i"]) {
            Ok(stdout) => {
                // server is runing
                if stdout.contains("STOP") {
                    // music is stopped, start playing music
                    let _ = execute_and_capture_output("mocp", &vec!["-p"]);
                } else {
                    // music is playing or paused, toggle play/pause
                    let _ = execute_and_capture_output("mocp", &vec!["-G"]);
                }
            }
            Err(e) => {
                status.force_clear();
                info!("mocp -i error:\n{e:?}");
                // server is stopped, start it.
                let c = execute_in_child("mocp", &vec!["-S"]);
                let Ok(mut c) = c else {
                    // it shouldn't fail, something is wrong. It's better not to do anything.
                    return Ok(())
                };
                let _ = c.wait();
                // start playing music
                let _ = execute_and_capture_output("mocp", &vec!["-p"]);
            }
        }
        Ok(())
    }

    /// Skip to the next song in MOC
    pub fn next() -> Result<()> {
        info!("mocp next");
        let _ = execute_in_child("mocp", &vec!["-f"]);
        Ok(())
    }

    /// Go to the previous song in MOC
    pub fn previous() -> Result<()> {
        info!("mocp previous");
        let _ = execute_in_child("mocp", &vec!["-r"]);
        Ok(())
    }
}
