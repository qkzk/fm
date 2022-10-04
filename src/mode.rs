use std::fmt;

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
    NeedConfirmation,
    /// Change the type of sort
    Sort,
    /// Preview a content with bat
    Preview,
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
            Mode::NeedConfirmation => write!(f, "Y/N   :"),
            Mode::Sort => write!(f, "(N)ame (D)ate (S)ize (E)xt (R)ev :"),
            Mode::Preview => write!(f, "Preview : "),
        }
    }
}
