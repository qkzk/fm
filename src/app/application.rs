use std::process::exit;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use clap::Parser;

use crate::app::{Displayer, Refresher, Status};
use crate::common::{clear_tmp_files, init_term, print_on_quit, CONFIG_PATH};
use crate::config::{cloud_config, load_config, set_configurable_static, Config, START_FOLDER};
use crate::event::{EventDispatcher, EventReader, FmEvents};
use crate::io::{set_loggers, Args, Opener};
use crate::log_info;

/// Holds everything about the application itself.
/// Most attributes holds an [`std::sync::Arc`] with [`tuikit::term::Term`].
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
    /// May fail if the [`tuikit::term`] can't be started or crashes
    pub fn start() -> Result<Self> {
        set_loggers()?;
        let Ok(config) = load_config(CONFIG_PATH) else {
            Self::exit_wrong_config()
        };

        let args = Args::parse();

        if args.keybinds {
            Self::exit_with_binds(&config);
        }

        if args.cloudconfig {
            Self::exit_with_cloud_config()?;
        }

        set_configurable_static(&args.path)?;
        Self::display_start_folder()?;

        let (fm_sender, fm_receiver) = std::sync::mpsc::channel::<FmEvents>();
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

    fn display_start_folder() -> Result<()> {
        let startfolder = START_FOLDER.get().context("Startfolder should be set")?;
        log_info!(
            "start folder: {startfolder}",
            startfolder = startfolder.display(),
        );
        Ok(())
    }

    fn exit_with_binds(config: &Config) {
        println!("{binds}", binds = config.binds.to_str());
        exit(0);
    }

    fn exit_with_cloud_config() -> Result<()> {
        cloud_config()?;
        exit(0);
    }

    /// Exit the application and log a message.
    /// Used when the config can't be read.
    fn exit_wrong_config() -> ! {
        eprintln!("Couldn't load the config file at {CONFIG_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/config.yaml for an example.");
        log_info!("Couldn't read the config file {CONFIG_PATH}");
        exit(1)
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
                status.attach_preview()?;
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

    /// Run the status loop and returns itself after completion.
    pub fn run(mut self) -> Result<Self> {
        while let Ok(event) = self.poll_event() {
            self.update(event)?;
            if self.must_quit()? {
                break;
            }
        }
        Ok(self)
    }

    /// Display the cursor,
    /// drop itself, which allow us to print normally afterward
    /// print the final path
    ///
    /// # Errors
    ///
    /// May fail if the terminal crashes
    /// May also fail if the thread running in [`crate::app::Refresher`] crashed
    pub fn quit(self) -> Result<()> {
        let final_path = self
            .status
            .lock()
            .map_err(|error| anyhow!("Error locking status: {error}"))?
            .current_tab_path_str()
            .to_owned();

        clear_tmp_files();
        drop(self.event_reader);
        drop(self.event_dispatcher);
        self.displayer.quit();
        self.refresher.quit();
        if let Ok(status) = self.status.lock() {
            status.previewer.quit()
        }
        drop(self.status);

        print_on_quit(final_path);
        Ok(())
    }
}
