/*
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

use nucleo::Nucleo;

pub enum NucleoKind {
    Path,
    Text,
    Help,
}

pub struct NucleoPicker {
    // /// the matcher
    // nucleo: Nucleo,
    /// what the user typed
    input_string: String,
    /// kind of matching
    kind: NucleoKind,
    /// last output from nucleo, as strings
    content: Vec<String>,
    /// currently selected index,
    index: usize,
    /// flagged files
    flagged: Vec<String>,
}
