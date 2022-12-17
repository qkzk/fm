use std::fmt;

#[derive(Clone)]
pub enum MarkAction {
    Jump,
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
    /// Execute a command on the file
    Exec,
    /// Display the help
    Help,
    /// Search in current directory for a string
    Search,
    /// cd into a directory
    Goto,
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
            Mode::Exec => write!(f, "Exec:    "),
            Mode::Help => write!(f, ""),
            Mode::Search => write!(f, "Search:  "),
            Mode::Goto => write!(f, "Goto  :  "),
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
