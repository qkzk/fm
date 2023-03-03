use std::fmt;

use crate::completion::InputCompleted;
use crate::cryptsetup::{EncryptedAction, PasswordKind};

/// Different kind of mark actions.
/// Either we jump to an existing mark or we save current path to a mark.
/// In both case, we'll have to listen to the next char typed.
#[derive(Clone, Copy)]
pub enum MarkAction {
    /// Jump to a selected mark (ie a path associated to a char)
    Jump,
    /// Creates a new mark (a path associated to a char)
    New,
}

/// Different kind of last edition command received requiring a confirmation.
/// Copy, move and delete require a confirmation to prevent big mistakes.
#[derive(Clone, Copy, Debug)]
pub enum NeedConfirmation {
    /// Copy flagged files
    Copy,
    /// Delete flagged files
    Delete,
    /// Move flagged files
    Move,
    /// Empty Trash
    EmptyTrash,
}

impl NeedConfirmation {
    /// Offset before the cursor.
    /// Since we ask the user confirmation, we need to know how much space
    /// is needed.
    pub fn cursor_offset(&self) -> usize {
        match *self {
            Self::Copy => 25,
            Self::Delete => 21,
            Self::Move => 25,
            Self::EmptyTrash => 35,
        }
    }
}

impl std::fmt::Display for NeedConfirmation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Delete => write!(f, "Delete files :"),
            Self::Move => write!(f, "Move files here :"),
            Self::Copy => write!(f, "Copy files here :"),
            Self::EmptyTrash => write!(f, "Empty the trash ?"),
        }
    }
}

/// Different modes in which the user is expeted to type something.
/// It may be a new filename, a mode (aka an octal permission),
/// the name of a new file, of a new directory,
/// A regex to match all files in current directory,
/// a kind of sort, a mark name, a new mark or a filter.
#[derive(Clone, Copy)]
pub enum InputSimple {
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
    /// Filter by extension, name, directory or no filter
    Filter,
    /// Set a new neovim RPC address
    SetNvimAddress,
    /// Input a password (chars a replaced by *)
    Password(PasswordKind, EncryptedAction),
}

/// Different modes in which we display a bunch of possible destinations.
/// In all those mode we can select a destination and move there.
#[derive(Clone, Copy)]
pub enum Navigate {
    /// Navigate to a flagged file
    Jump,
    /// Navigate back to a visited path
    History,
    /// Navigate to a predefined shortcut
    Shortcut,
    /// Manipulate a trash file
    Trash,
    /// Manipulate an encrypted device
    EncryptedDrive,
    /// Jump to a saved mark
    Marks(MarkAction),
    /// Pick a compression method
    Compress,
    ///
    Bulk,
}

/// Different mode in which the application can be.
/// It dictates the reaction to event and what to display.
#[derive(Clone, Copy)]
pub enum Mode {
    /// Default mode: display the files
    Normal,
    /// Display files in a tree
    Tree,
    /// We'll be able to complete the input string with
    /// different kind of completed items (exec, goto, search)
    InputCompleted(InputCompleted),
    /// Select a target and navigate to it
    Navigate(Navigate),
    /// Confirmation is required before modification is made to existing files :
    /// delete, move, copy
    NeedConfirmation(NeedConfirmation),
    /// Preview a file content
    Preview,
    /// Modes requiring an input that can't be completed
    InputSimple(InputSimple),
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Mode::Normal => write!(f, "Normal:  "),
            Mode::Tree => write!(f, "Tree:    "),
            Mode::InputSimple(InputSimple::Rename) => write!(f, "Rename:  "),
            Mode::InputSimple(InputSimple::Chmod) => write!(f, "Chmod:   "),
            Mode::InputSimple(InputSimple::Newfile) => write!(f, "Newfile: "),
            Mode::InputSimple(InputSimple::Newdir) => write!(f, "Newdir:  "),
            Mode::InputSimple(InputSimple::RegexMatch) => write!(f, "Regex:   "),
            Mode::InputSimple(InputSimple::SetNvimAddress) => write!(f, "Neovim:  "),
            Mode::InputSimple(InputSimple::Sort) => {
                write!(f, "Sort: Kind Name Modif Size Ext Rev :")
            }
            Mode::Navigate(Navigate::Marks(_)) => write!(f, "Marks jump:"),
            Mode::InputSimple(InputSimple::Filter) => write!(f, "Filter:  "),
            Mode::InputSimple(InputSimple::Password(password_kind, _)) => {
                write!(f, "{password_kind}")
            }
            Mode::InputCompleted(InputCompleted::Exec) => write!(f, "Exec:    "),
            Mode::InputCompleted(InputCompleted::Goto) => write!(f, "Goto  :  "),
            Mode::InputCompleted(InputCompleted::Search) => write!(f, "Search:  "),
            Mode::InputCompleted(InputCompleted::Nothing) => write!(f, "Nothing:  "),
            Mode::InputCompleted(InputCompleted::Command) => write!(f, "Command:  "),
            Mode::Navigate(Navigate::Jump) => write!(f, "Jump  :  "),
            Mode::Navigate(Navigate::History) => write!(f, "History :"),
            Mode::Navigate(Navigate::Shortcut) => write!(f, "Shortcut :"),
            Mode::Navigate(Navigate::Trash) => write!(f, "Trash    :"),
            Mode::Navigate(Navigate::Bulk) => write!(f, "Bulk     :"),
            Mode::Navigate(Navigate::Compress) => write!(f, "Compress :"),
            Mode::Navigate(Navigate::EncryptedDrive) => {
                write!(f, "Encrypted devices :")
            }
            Mode::NeedConfirmation(_) => write!(f, "Y/N   :"),
            Mode::Preview => write!(f, "Preview : "),
        }
    }
}
