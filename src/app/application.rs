use std::sync::Arc;
use std::sync::Mutex;

use anyhow::anyhow;
use anyhow::Result;

use crate::app::Displayer;
use crate::app::Refresher;
use crate::app::Status;
use crate::common::CONFIG_PATH;
use crate::common::{clear_tmp_file, init_term};
use crate::config::load_config;
use crate::config::START_FOLDER;
use crate::event::EventDispatcher;
use crate::event::EventReader;
use crate::event::FmEvents;
use crate::io::set_loggers;
use crate::io::Opener;
use crate::log_info;

/// Holds everything about the application itself.
/// Most attributes holds an `Arc<tuikit::Term::term>`.
/// Dropping the instance of FM allows to write again to stdout.
pub struct FM {
    /// Poll the event sent to the terminal by the user or the OS
    event_reader: EventReader,
    /// Associate the event to a method, modifing the status.
    event_dispatcher: EventDispatcher,
    /// Current status of the application. Mostly the filetrees
    status: Arc<Mutex<Status>>,
    /// Refresher is used to force a refresh when a file has been modified externally.
    /// It sends an `Event::Key(Key::AltPageUp)` every 10 seconds.
    /// It also has a `mpsc::Sender` to send a quit message and reset the cursor.
    refresher: Refresher,
    /// Used to handle every display on the screen, except from skim (fuzzy finds).
    /// It runs a single thread with an mpsc receiver to handle quit events.
    /// Drawing is done 30 times per second.
    displayer: Displayer,
}

impl FM {
    /// Setup everything the application needs in its main loop :
    /// an `EventReader`,
    /// an `EventDispatcher`,
    /// a `Status`,
    /// a `Display`,
    /// a `Refresher`.
    /// It reads and drops the configuration from the config file.
    /// If the config can't be parsed, it exits with error code 1.
    ///
    /// # Errors
    ///
    /// May fail if the [`tuikit::prelude::term`] can't be started or crashes
    pub fn start() -> Result<Self> {
        let (fm_sender, fm_receiver) = std::sync::mpsc::channel::<FmEvents>();
        set_loggers()?;
        let Ok(config) = load_config(CONFIG_PATH) else {
            exit_wrong_config()
        };
        log_info!(
            "start folder: {startfolder}",
            startfolder = &START_FOLDER.display()
        );
        let term = Arc::new(init_term()?);
        let fm_sender = Arc::new(fm_sender);
        let event_reader = EventReader::new(term.clone(), fm_receiver);
        let event_dispatcher = EventDispatcher::new(config.binds.clone());
        let opener = Opener::new(&config.terminal, &config.terminal_flag);
        let status = Arc::new(Mutex::new(Status::new(
            term.term_size()?.1,
            term.clone(),
            opener,
            &config.binds,
            fm_sender.clone(),
        )?));
        drop(config);

        // let refresher = Refresher::new(term.clone());
        let refresher = Refresher::new(fm_sender);
        let displayer = Displayer::new(term, status.clone());
        Ok(Self {
            event_reader,
            event_dispatcher,
            status,
            refresher,
            displayer,
        })
    }

    /// Return the last event received by the terminal
    ///
    /// # Errors
    ///
    /// May fail if the terminal crashes
    fn poll_event(&self) -> Result<FmEvents> {
        self.event_reader.poll_event()
    }

    /// Update itself, changing its status.
    fn update(&mut self, event: FmEvents) -> Result<()> {
        match self.status.lock() {
            Ok(mut status) => {
                self.event_dispatcher.dispatch(&mut status, event)?;
                status.refresh_shortcuts();
                drop(status);
                Ok(())
            }
            Err(error) => Err(anyhow!("Error locking status: {error}")),
        }
    }

    /// True iff the application must quit.
    fn must_quit(&self) -> Result<bool> {
        match self.status.lock() {
            Ok(status) => Ok(status.must_quit()),
            Err(error) => Err(anyhow!("Error locking status: {error}")),
        }
    }

    /// Run the status loop.
    pub fn run(&mut self) -> Result<()> {
        while let Ok(event) = self.poll_event() {
            self.update(event)?;
            if self.must_quit()? {
                break;
            }
        }
        Ok(())
    }

    /// Display the cursor,
    /// drop itself, which allow us to print normally afterward
    /// print the final path
    ///
    /// # Errors
    ///
    /// May fail if the terminal crashes
    /// May also fail if the thread running in [`crate::application::Refresher`] crashed
    pub fn quit(self) -> Result<String> {
        clear_tmp_file();
        drop(self.event_reader);
        drop(self.event_dispatcher);
        self.displayer.quit()?;
        self.refresher.quit()?;

        match self.status.lock() {
            Ok(status) => {
                let final_path = status.current_tab_path_str().to_owned();
                drop(status);
                Ok(final_path)
            }
            Err(error) => Err(anyhow!("Error locking status {error}")),
        }
    }
}

/// Exit the application and log a message.
/// Used when the config can't be read.
fn exit_wrong_config() -> ! {
    eprintln!("Couldn't load the config file at {CONFIG_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/config.yaml for an example.");
    log_info!("Couldn't read the config file {CONFIG_PATH}");
    std::process::exit(1)
}

// /// Force clear the display if the status requires it, then reset it in status.
// ///
// /// # Errors
// ///
// /// May fail if the terminal crashes
// fn force_clear_if_needed(a_status: bool) -> Result<()> {
//     let mut status = a_status.lock().unwrap();
//     if status.internal_settings.force_clear {
//         self.display.force_clear()?;
//         status.internal_settings.force_clear = false;
//     }
//     Ok(())
// }

// /// Display itself using its `display` attribute.
// ///
// /// # Errors
// ///
// /// May fail if the terminal crashes
// /// The display itself may fail if it encounters unreadable file in preview mode
// fn display(&mut self, status: MutexGuard<Status>) -> Result<()> {
// self.force_clear_if_needed()?;
// self.display.display_all(status)
// }
