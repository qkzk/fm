use std::fs::{read_dir, remove_file};
use std::io::stdout;
use std::panic;
use std::process::exit;
use std::sync::{mpsc, Arc, Mutex};

#[cfg(debug_assertions)]
use std::backtrace;

use anyhow::{anyhow, bail, Result};
use clap::Parser;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::disable_raw_mode,
};
use ratatui::{init as init_term, DefaultTerminal};

use crate::app::{Displayer, Refresher, Status};
use crate::common::{clear_tmp_files, save_final_path, CONFIG_PATH, TMP_THUMBNAILS_DIR};
use crate::config::{cloud_config, load_config, set_configurable_static, Config};
use crate::event::{EventDispatcher, EventReader, FmEvents};
use crate::io::{set_loggers, Args, Opener};
use crate::log_info;

/// Holds everything about the application itself.
/// Dropping the instance of FM allows to write again to stdout.
/// It should be ran like this : `crate::app::Fm::start().run().quit()`.
pub struct FM {
    /// Poll the event sent to the terminal by the user or the OS
    event_reader: EventReader,
    /// Associate the event to a method, modifing the status.
    event_dispatcher: EventDispatcher,
    /// Current status of the application. Mostly the filetrees
    status: Arc<Mutex<Status>>,
    /// Refresher is used to force a refresh when a file has been modified externally.
    /// It also has a [`std::mpsc::Sender`] to send a quit message and reset the cursor.
    refresher: Refresher,
    /// Used to handle every display on the screen, except from skim (fuzzy finds).
    /// It runs a single thread with an mpsc receiver to handle quit events.
    /// Drawing is done 30 times per second.
    displayer: Displayer,
}

impl FM {
    /// Setup everything the application needs in its main loop :
    /// a panic hook for graceful panic and displaying a traceback for debugging purpose,
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
    /// May fail if the [`ratatui::DefaultTerminal`] can't be started or crashes
    pub fn start() -> Result<Self> {
        Self::set_panic_hook();
        let (config, start_folder) = Self::early_exit()?;
        log_info!("start folder: {start_folder}");
        set_configurable_static(&start_folder)?;
        Self::build(config)
    }

    /// Set a panic hook for debugging the application.
    /// In case of panic, we ensure to:
    /// - erase temporary files
    /// - restore the terminal as best as possible (show the cursor, disable the mouse capture)
    /// - if in debug mode (target=debug), display a full traceback.
    /// - if in release mode (target=release), display a sorry message.
    fn set_panic_hook() {
        panic::set_hook(Box::new(|traceback| {
            clear_tmp_files();
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), cursor::Show, DisableMouseCapture);

            if cfg!(debug_assertions) {
                if let Some(payload) = traceback.payload().downcast_ref::<&str>() {
                    eprintln!("Traceback: {payload}",);
                } else if let Some(payload) = traceback.payload().downcast_ref::<String>() {
                    eprintln!("Traceback: {payload}",);
                } else {
                    eprintln!("Traceback:{traceback:?}");
                }
                if let Some(location) = traceback.location() {
                    eprintln!("At {location}");
                }
                #[cfg(debug_assertions)]
                eprintln!("{}", backtrace::Backtrace::capture());
            } else {
                eprintln!("fm exited unexpectedly.");
            }
        }));
    }

    /// Read config and args, leaving immediatly if the arguments say so.
    /// It will return the fully set [`crate::fm::config::Config`] and the starting path
    /// as a String.
    fn early_exit() -> Result<(Config, String)> {
        let args = Args::parse();
        if args.log {
            set_loggers()?;
        }
        let Ok(config) = load_config(CONFIG_PATH) else {
            Self::exit_wrong_config()
        };
        if args.keybinds {
            Self::exit_with_binds(&config);
        }
        if args.cloudconfig {
            Self::exit_with_cloud_config()?;
        }
        if args.clear_cache {
            Self::exit_with_clear_cache()?;
        }
        Ok((config, args.path))
    }

    fn exit_with_binds(config: &Config) {
        println!("{binds}", binds = config.binds.to_str());
        exit(0);
    }

    fn exit_with_cloud_config() -> Result<()> {
        cloud_config()?;
        exit(0);
    }

    fn exit_with_clear_cache() -> Result<()> {
        read_dir(TMP_THUMBNAILS_DIR)?
            .filter_map(|entry| entry.ok())
            .for_each(|e| {
                if let Err(e) = remove_file(e.path()) {
                    println!("Couldn't remove {TMP_THUMBNAILS_DIR}: error {e}");
                }
            });
        println!("Cleared {TMP_THUMBNAILS_DIR}");
        exit(0);
    }

    /// Exit the application and log a message.
    /// Used when the config can't be read.
    fn exit_wrong_config() -> ! {
        eprintln!("Couldn't load the config file at {CONFIG_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/config.yaml for an example.");
        log_info!("Couldn't read the config file {CONFIG_PATH}");
        exit(1)
    }

    /// Internal builder. Builds an Fm instance from the config.
    fn build(config: Config) -> Result<Self> {
        let (fm_sender, fm_receiver) = mpsc::channel::<FmEvents>();
        let fm_sender = Arc::new(fm_sender);
        let term = Self::init_term();
        let event_reader = EventReader::new(fm_receiver);
        let event_dispatcher = EventDispatcher::new(config.binds.clone());
        let status = Arc::new(Mutex::new(Status::new(
            term.size().unwrap(),
            Opener::new(&config.terminal, &config.terminal_flag),
            &config.binds,
            fm_sender.clone(),
        )?));
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

    fn init_term() -> DefaultTerminal {
        let term = init_term();
        execute!(stdout(), EnableMouseCapture).unwrap();
        term
    }

    /// Update itself, changing its status.
    /// It will dispatch every [`FmEvents`], updating [`Status`].
    fn update(&mut self, event: FmEvents) -> Result<()> {
        let Ok(mut status) = self.status.lock() else {
            bail!("Error locking status");
        };
        self.event_dispatcher.dispatch(&mut status, event)?;

        Ok(())
    }

    /// True iff the application must quit.
    fn must_quit(&self) -> Result<bool> {
        let Ok(status) = self.status.lock() else {
            bail!("Error locking status");
        };
        Ok(status.must_quit())
    }

    /// Run the update status loop and returns itself after completion.
    pub fn run(mut self) -> Result<Self> {
        while !self.must_quit()? {
            self.update(self.event_reader.poll_event())?;
        }
        Ok(self)
    }

    /// Disable the mouse capture and clear before normal exit.
    fn disable_mouse_capture_and_clear() -> Result<()> {
        execute!(stdout(), Clear(ClearType::All), DisableMouseCapture)?;
        Ok(())
    }

    /// Reset everything as best as possible, stop any long thread in a loop and exit.
    ///
    /// More specifically :
    /// - Display the cursor,
    /// - drop itself, which allow us to print normally afterward
    /// - print the final path
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
            status.previewer.quit();
        }
        drop(self.status);
        Self::disable_mouse_capture_and_clear()?;
        save_final_path(&final_path);
        Ok(())
    }
}
