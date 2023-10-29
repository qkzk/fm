use std::borrow::BorrowMut;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use fm::preview::PreviewArgs;
use log::info;

use fm::config::{load_config, Colors};
use fm::constant_strings_paths::{CONFIG_PATH, OPENER_PATH};
use fm::event_dispatch::EventDispatcher;
use fm::help::Help;
use fm::log::set_loggers;
use fm::opener::{load_opener, Opener};
use fm::status::Status;
use fm::term_manager::{Display, EventReader};
use fm::utils::{clear_tmp_file, init_term, print_on_quit};

/// Holds everything about the application itself.
/// Most attributes holds an `Arc<tuiki::Term::term>`.
/// Dropping the instance of FM allows to write again to stdout.
struct FM {
    /// Poll the event sent to the terminal by the user or the OS
    event_reader: EventReader,
    /// Associate the event to a method, modifing the status.
    event_dispatcher: EventDispatcher,
    /// Current status of the application. Mostly the filetrees
    status: Status,
    /// Responsible for the display on screen.
    display: Display,
    /// Colors used by different kind of files.
    /// Since most are generated the first time an extension is met,
    /// we need to hold this.
    colors: Colors,
    /// Refresher is used to force a refresh when a file has been modified externally.
    /// It send `Event::Key(Key::AltPageUp)` every 10 seconds.
    /// It also has a `mpsc::Sender` to send a quit message and reset the cursor.
    refresher: Refresher,
    previewer: Previewer,
    rx2: mpsc::Receiver<Option<fm::preview::Preview>>,
}

impl FM {
    /// Setup everything the application needs in its main loop :
    /// an `EventReader`,
    /// an `EventDispatcher`,
    /// a `Status`,
    /// a `Display`,
    /// some `Colors`,
    /// a `Refresher`.
    /// It reads and drops the configuration from the config file.
    /// If the config can't be parsed, it exits with error code 1.
    fn start() -> Result<Self> {
        let Ok(config) = load_config(CONFIG_PATH) else {
            exit_wrong_config()
        };
        let term = Arc::new(init_term()?);
        let event_reader = EventReader::new(Arc::clone(&term));
        let event_dispatcher = EventDispatcher::new(config.binds.clone());
        let opener = load_opener(OPENER_PATH, &config.terminal).unwrap_or_else(|_| {
            eprintln!("Couldn't read the opener config file at {OPENER_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/opener.yaml for an example. Using default.");
            info!("Couldn't read opener file at {OPENER_PATH}. Using default.");
            Opener::new(&config.terminal)
        });
        let help = Help::from_keybindings(&config.binds, &opener)?.help;
        let display = Display::new(Arc::clone(&term));
        let status = Status::new(
            display.height()?,
            Arc::clone(&term),
            help,
            opener,
            &config.settings,
        )?;
        let colors = config.colors.clone();
        let refresher = Refresher::spawn(term);
        let (previewer, rx2) = Previewer::spawn();
        drop(config);
        Ok(Self {
            event_reader,
            event_dispatcher,
            status,
            display,
            colors,
            refresher,
            previewer,
            rx2,
        })
    }

    /// Return the last event received by the terminal
    fn poll_event(&self) -> Result<tuikit::prelude::Event> {
        self.event_reader.poll_event()
    }

    /// Force clear the display if the status requires it, then reset it in status.
    fn force_clear_if_needed(&mut self) -> Result<()> {
        if self.status.force_clear {
            self.display.force_clear()?;
            self.status.force_clear = false;
        }
        Ok(())
    }

    /// Update itself, changing its status.
    fn update(&mut self, event: tuikit::prelude::Event) -> Result<()> {
        self.event_dispatcher.dispatch(
            &mut self.status,
            event,
            &self.colors,
            self.event_reader.term_height()?,
        )?;
        self.status.refresh_disks();
        Ok(())
    }

    /// Display itself using its `display` attribute.
    fn display(&mut self) -> Result<()> {
        self.force_clear_if_needed()?;
        self.display.display_all(&self.status, &self.colors)
    }

    /// True iff the application must quit.
    fn must_quit(&self) -> bool {
        self.status.must_quit()
    }

    /// Display the cursor,
    /// drop itself, which allow us to print normally afterward
    /// print the final path
    fn quit(self) -> Result<()> {
        clear_tmp_file();
        self.display.show_cursor()?;
        let final_path = self.status.selected_path_str().to_owned();
        self.refresher.quit()?;
        print_on_quit(&final_path);
        info!("fm is shutting down");
        Ok(())
    }
}

/// Allows refresh if the current path has been modified externally.
struct Refresher {
    /// Sender of messages, used to terminate the thread properly
    tx: mpsc::Sender<()>,
    /// Handle to the `term::Event` sender thread.
    handle: thread::JoinHandle<()>,
}

impl Refresher {
    /// Spawn a thread which sends events to the terminal.
    /// Those events are interpreted as refresh requests.
    /// It also listen to a receiver for quit messages.
    ///
    /// This will send periodically an `Key::AltPageUp` event to the terminal which requires a refresh.
    /// This keybind is reserved and can't be bound to anything.
    ///
    /// Using Event::User(()) conflicts with skim internal which interpret this
    /// event as a signal(1) and hangs the terminal.
    fn spawn(mut term: Arc<tuikit::term::Term>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut counter: u8 = 0;
        let handle = thread::spawn(move || loop {
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Disconnected) => {
                    log::info!("terminating refresher");
                    let _ = term.show_cursor(true);
                    return;
                }
                Err(TryRecvError::Empty) => {}
            }
            counter += 1;
            thread::sleep(Duration::from_millis(100));
            if counter >= 10 * 10 {
                counter = 0;
                let event = tuikit::prelude::Event::Key(tuikit::prelude::Key::AltPageUp);
                if term.borrow_mut().send_event(event).is_err() {
                    break;
                }
            }
        });
        Self { tx, handle }
    }

    /// Send a quit message to the receiver, signaling it to quit.
    /// Join the refreshing thread which should be terminated.
    fn quit(self) -> Result<()> {
        self.tx.send(())?;
        let _ = self.handle.join();
        Ok(())
    }
}

/// Allows refresh if the current path has been modified externally.
struct Previewer {
    /// Sender of messages, used to terminate the thread properly
    tx: mpsc::Sender<Option<PreviewArgs>>,
    /// Handle to the `term::Event` sender thread.
    handle: thread::JoinHandle<()>,
}

impl Previewer {
    /// Spawn a thread which sends events to the terminal.
    /// Those events are interpreted as refresh requests.
    /// It also listen to a receiver for quit messages.
    ///
    /// This will send periodically an `Key::AltPageUp` event to the terminal which requires a refresh.
    /// This keybind is reserved and can't be bound to anything.
    ///
    /// Using Event::User(()) conflicts with skim internal which interpret this
    /// event as a signal(1) and hangs the terminal.
    fn spawn() -> (Self, mpsc::Receiver<Option<fm::preview::Preview>>) {
        let (tx, rx) = mpsc::channel::<Option<PreviewArgs>>();
        let (tx2, rx2) = mpsc::channel::<Option<fm::preview::Preview>>();
        let handle = thread::spawn(move || loop {
            match rx.try_recv() {
                Ok(preview_args) => {
                    let preview = Self::make_preview(preview_args);
                    tx2.send(preview).unwrap();
                }
                Err(TryRecvError::Disconnected) => {
                    log::info!("terminating previewer");
                    return;
                }
                Err(TryRecvError::Empty) => {}
            }
        });
        (Self { tx, handle }, rx2)
    }

    fn make_preview(preview_args: Option<PreviewArgs>) -> Option<fm::preview::Preview> {
        if let Some(preview_args) = preview_args {
            let res = fm::preview::Preview::new(&preview_args.fileinfo);
            if let Ok(preview) = res {
                Some(preview)
            } else {
                None
            }
        } else {
            None
        }
    }
}
/// Exit the application and log a message.
/// Used when the config can't be read.
fn exit_wrong_config() -> ! {
    eprintln!("Couldn't load the config file at {CONFIG_PATH}. See https://raw.githubusercontent.com/qkzk/fm/master/config_files/fm/config.yaml for an example.");
    info!("Couldn't read the config file {CONFIG_PATH}");
    std::process::exit(1)
}

/// Main function
/// Init the status and display and listen to events (keyboard, mouse, resize, custom...).
/// The application is redrawn after every event.
/// When the user issues a quit event, the main loop is broken
/// Then we reset the cursor, drop everything holding a terminal and print the last path.
fn main() -> Result<()> {
    set_loggers()?;
    let mut fm = FM::start()?;

    while let Ok(event) = fm.poll_event() {
        fm.update(event)?;
        if let Some(fileinfo) = fm.status.tabs[0].selected() {
            let preview_args = PreviewArgs::new(fileinfo);
            fm.previewer.tx.send(Some(preview_args))?;
            match fm.rx2.try_recv() {
                Ok(preview) => {
                    if let Some(preview) = preview {
                        log::info!("rx2 received {path}", path = preview.path().display());
                        fm.status.update_preview(preview);
                    }
                }
                _ => (),
            }
        }
        fm.display()?;
        if fm.must_quit() {
            break;
        }
    }

    fm.quit()
}
