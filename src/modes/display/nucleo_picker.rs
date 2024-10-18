/*

It's a full screen with custom binds

So... it must capture all key events and handle them at root level of dispatch
I may want to use a fork of nucleo-picker, it might be easier to maintain
I'll just have to allow :

- sending a terminal and not creating one
- receiving events from MPSC instead of "event::poll() event::read()"
- capturing the display ?

plan:

1. fork
2. allow "term" to be sent from somewhere else... use it ?

3. lock the term to be passed to Picker
4. let the picker to the magic
4. capture its result in caller


+-------------------+
|   Cargo           | <----
|                   |
|    ./Cargo.toml   |
| >   ./Cargo.lock  |
|                   |
|                   |
|                   |
+-------------------+

typing something sends a message to nucleo
we call snapshot... to get an updated list
space selects an element

ENTER returns the selected + flagged elements

many questions :

- how to know where we were ?

need at least one MPSC ?

we look for a path from current dir

users types "a"

the new prompt is sent, the output is displayed
I should only use nucleo-picker as a reference and adapt to what I'm doing.

I could :

lock the terminal
do a picker whatever
send its output as Vec<String> through messages or whatever
unlock the terminal
and voila ... would it work ?
I may have conflicts with normal message dispatch... since they're read from a global function... check the mode, send it back there

*/

use std::{io, thread::spawn};

use nucleo_picker::{nucleo::Config, Picker};

static TEXT: [&str; 4] = ["man", "woman", "camera", "tv"];

pub fn nucleo() -> io::Result<()> {
    // See the nucleo configuration for more options:
    //   https://docs.rs/nucleo/latest/nucleo/struct.Config.html
    let config = Config::DEFAULT.match_paths();

    // Initialize a picker with the provided configuration
    let mut picker = Picker::with_config(config).without_reset();

    // "argument parsing"

    // populate from a separate thread to avoid locking the picker interface
    let injector = picker.injector();
    spawn(move || {
        for entry in TEXT.into_iter() {
            let _ = injector.push(entry, |e, cols| {
                // the picker only has one column; fill it with the match text
                cols[0] = e.to_string().into();
            });
        }
    });

    match picker.pick()? {
        Some(entry) => {
            // the matched `entry` is &DirEntry
            crate::log_info!("Selected : '{}'", entry);
        }
        None => {
            crate::log_info!("Nothing selected!");
        }
    }

    Ok(())
}
