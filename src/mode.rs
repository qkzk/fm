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

/// Different kind of last edition command received requiring a confirmation.
/// Copy, move and delete require a confirmation to prevent big mistakes.
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

#[derive(Clone)]
pub enum InputKind {
    /// Rename the selected file
    Rename,
    /// Change permissions of the selected file
    Chmod,
    /// Touch a new file
    Newfile,
    /// Mkdir a new directory
    Newdir,
    /// Flag files matching a regex
    RegexMatch,
    /// Change the type of sort
    Sort,
    /// Jump to a saved mark
    Marks(MarkAction),
    /// Filter by extension, name, directory or no filter
    Filter,
}

/// Different mode in which the application can be.
/// It dictates the reaction to event and what to display.
#[derive(Clone)]
pub enum Mode {
    /// Default mode: display the files
    Normal,
    /// We'll be able to complete the input string with
    /// different kind of completed items (exec, goto, search)
    Completed(CompletionKind),
    /// Display the help
    Help,
    /// Jump to a flagged file
    Jump,
    /// Confirmation is required before modification is made to existing files :
    /// delete, move, copy
    NeedConfirmation(ConfirmedAction),
    /// Preview a file content
    Preview,
    /// Display a sort of stack of visited directories
    History,
    /// Display predefined shortcuts
    Shortcut,
    /// Modes requiring an input that can't be completed
    ReadInput(InputKind),
}

impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Mode::Normal => write!(f, "Normal:  "),
            Mode::ReadInput(InputKind::Rename) => write!(f, "Rename:  "),
            Mode::ReadInput(InputKind::Chmod) => write!(f, "Chmod:   "),
            Mode::ReadInput(InputKind::Newfile) => write!(f, "Newfile: "),
            Mode::ReadInput(InputKind::Newdir) => write!(f, "Newdir:  "),
            Mode::ReadInput(InputKind::RegexMatch) => write!(f, "Regex :  "),
            Mode::ReadInput(InputKind::Sort) => write!(f, "Sort: Kind Name Modif Size Ext Rev :"),
            Mode::ReadInput(InputKind::Marks(_)) => write!(f, "Marks jump:"),
            Mode::ReadInput(InputKind::Filter) => write!(f, "Filter:  "),
            Mode::Completed(CompletionKind::Exec) => write!(f, "Exec:    "),
            Mode::Completed(CompletionKind::Goto) => write!(f, "Goto  :  "),
            Mode::Completed(CompletionKind::Search) => write!(f, "Search:  "),
            Mode::Completed(CompletionKind::Nothing) => write!(f, "Nothing:  "),
            Mode::Help => write!(f, ""),
            Mode::Jump => write!(f, "Jump  :  "),
            Mode::History => write!(f, "History :"),
            Mode::NeedConfirmation(_) => write!(f, "Y/N   :"),
            Mode::Preview => write!(f, "Preview : "),
            Mode::Shortcut => write!(f, "Shortcut :"),
        }
    }
}
