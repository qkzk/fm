use std::sync::Arc;

use anyhow::Result;
use tuikit::prelude::Event;

use crate::app::Refresher;
use crate::app::Status;
use crate::common::CONFIG_PATH;
use crate::common::{clear_tmp_file, init_term, print_on_quit};
use crate::config::load_config;
use crate::config::START_FOLDER;
use crate::event::EventDispatcher;
use crate::event::EventReader;
use crate::io::set_loggers;
use crate::io::Display;
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
    status: Status,
    /// Responsible for the display on screen.
    display: Display,
    /// Refresher is used to force a refresh when a file has been modified externally.
    /// It sends an `Event::Key(Key::AltPageUp)` every 10 seconds.
    /// It also has a `mpsc::Sender` to send a quit message and reset the cursor.
    refresher: Refresher,
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
        set_loggers()?;
        let Ok(config) = load_config(CONFIG_PATH) else {
            exit_wrong_config()
        };
        log_info!(
            "start folder: {startfolder}",
            startfolder = &START_FOLDER.display()
        );
        let term = Arc::new(init_term()?);
        let event_reader = EventReader::new(Arc::clone(&term));
        let event_dispatcher = EventDispatcher::new(config.binds.clone());
        let opener = Opener::new(&config.terminal);
        let display = Display::new(Arc::clone(&term));
        let status = Status::new(
            display.height()?,
            Arc::clone(&term),
            opener,
            &config.settings,
        )?;
        let refresher = Refresher::new(term);
        drop(config);
        Ok(Self {
            event_reader,
            event_dispatcher,
            status,
            display,
            refresher,
        })
    }

    /// Return the last event received by the terminal
    ///
    /// # Errors
    ///
    /// May fail if the terminal crashes
    pub fn poll_event(&self) -> Result<Event> {
        self.event_reader.poll_event()
    }

    /// Force clear the display if the status requires it, then reset it in status.
    ///
    /// # Errors
    ///
    /// May fail if the terminal crashes
    pub fn force_clear_if_needed(&mut self) -> Result<()> {
        if self.status.force_clear {
            self.display.force_clear()?;
            self.status.force_clear = false;
        }
        Ok(())
    }

    /// Update itself, changing its status.
    pub fn update(&mut self, event: Event) -> Result<()> {
        self.event_dispatcher.dispatch(&mut self.status, event)?;
        self.status.refresh_disks();
        Ok(())
    }

    /// Display itself using its `display` attribute.
    ///
    /// # Errors
    ///
    /// May fail if the terminal crashes
    /// The display itself may fail if it encounters unreadable file in preview mode
    pub fn display(&mut self) -> Result<()> {
        self.force_clear_if_needed()?;
        self.display.display_all(&self.status)
    }

    /// True iff the application must quit.
    pub fn must_quit(&self) -> bool {
        self.status.must_quit()
    }

    /// Display the cursor,
    /// drop itself, which allow us to print normally afterward
    /// print the final path
    ///
    /// # Errors
    ///
    /// May fail if the terminal crashes
    /// May also fail if the thread running in [`crate::application::Refresher`] crashed
    pub fn quit(self) -> Result<()> {
        clear_tmp_file();
        self.display.show_cursor()?;
        let final_path = self.status.selected_path_str().to_owned();
        self.refresher.quit()?;
        drop(self.event_reader);
        drop(self.event_dispatcher);
        drop(self.display);
        drop(self.status);
        print_on_quit(&final_path);
        log_info!("fm is shutting down");
        Ok(())
    }
}

/// Exit the application and log a message.
/// Used when the config can't be read.
fn exit_wrong_config() -> ! {
    eprintln!("Couldn't load the config file at {CONFIG_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/config.yaml for an example.");
    log_info!("Couldn't read the config file {CONFIG_PATH}");
    std::process::exit(1)
}
