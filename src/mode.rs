use std::fmt;

use crate::completion::CompletionKind;

/// Different kind of mark actions.
/// Either we jump to an existing mark or we save current path to a mark.
/// In both case, we'll have to listen to the next char typed.
#[derive(Clone)]
pub enum MarkAction {
    /// Jump to a selected mark (ie a path associated to a char)
    Jump,
    /// Creates a new mark (a path associated to a char)
    New,
}

/// Different kind of last edition command received.
#[derive(Clone, Copy, Debug)]
pub enum ConfirmedAction {
    /// Copy flagged files
    Copy,
    /// Delete flagged files
    Delete,
    /// Move flagged files
    Move,
}

impl ConfirmedAction {
    /// Offset before the cursor.
    /// Since we ask the user confirmation, we need to know how much space
    /// is needed.
    pub fn cursor_offset(&self) -> usize {
        match *self {
            Self::Copy => 25,
            Self::Delete => 21,
            Self::Move => 25,
        }
    }
}

impl std::fmt::Display for ConfirmedAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Delete => write!(f, "Delete files :"),
            Self::Move => write!(f, "Move files here :"),
            Self::Copy => write!(f, "Copy files here :"),
        }
    }
}

/// Different mode in which the application can be.
/// It dictates the reaction to event and what to display.
#[derive(Clone)]
pub enum Mode {
    /// Default mode: display the files
    Normal,
    /// Rename the selected file
    Rename,
    /// Change permissions of the selected file
    Chmod,
    /// Touch a new file
    Newfile,
    /// Mkdir a new directory
    Newdir,
    /// We'll be able to complete the input string with
    /// different kind of completed items (exec, goto, search)
    Completed(CompletionKind),
    /// Display the help
    Help,
    /// Flag files matching a regex
    RegexMatch,
    /// Jump to a flagged file
    Jump,
    /// Confirmation is required before modification is made to files :
    /// delete, move, copy
    NeedConfirmation(ConfirmedAction),
    /// Change the type of sort
    Sort,
    /// Preview a file content
    Preview,
    /// Display a sort of stack of visited directories
    History,
    /// Display predefined shortcuts
    Shortcut,
    /// Jump to a saved mark
    Marks(MarkAction),
    /// Filter by extension, name, directory or no filter
    Filter,
}

impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Mode::Normal => write!(f, "Normal:  "),
            Mode::Rename => write!(f, "Rename:  "),
            Mode::Chmod => write!(f, "Chmod:   "),
            Mode::Newfile => write!(f, "Newfile: "),
            Mode::Newdir => write!(f, "Newdir:  "),
            Mode::Completed(CompletionKind::Exec) => write!(f, "Exec:    "),
            Mode::Completed(CompletionKind::Goto) => write!(f, "Goto  :  "),
            Mode::Completed(CompletionKind::Search) => write!(f, "Search:  "),
            Mode::Completed(CompletionKind::Nothing) => write!(f, "Nothing:  "),
            Mode::Help => write!(f, ""),
            Mode::RegexMatch => write!(f, "Regex :  "),
            Mode::Jump => write!(f, "Jump  :  "),
            Mode::History => write!(f, "History :"),
            Mode::NeedConfirmation(_) => write!(f, "Y/N   :"),
            Mode::Sort => write!(f, "Sort: Kind Name Modif Size Ext Rev :"),
            Mode::Preview => write!(f, "Preview : "),
            Mode::Shortcut => write!(f, "Shortcut :"),
            Mode::Marks(_) => write!(f, "Marks jump:"),
            Mode::Filter => write!(f, "Filter:  "),
        }
    }
}
