use std::fmt;

use crate::completion::InputCompleted;
use crate::constant_strings_paths::{
    CHMOD_LINES, FILTER_LINES, NEWDIR_LINES, NEWFILE_LINES, NVIM_ADDRESS_LINES,
    PASSWORD_LINES_DEVICE, PASSWORD_LINES_SUDO, REGEX_LINES, REMOTE_LINES, RENAME_LINES,
    SHELL_LINES, SORT_LINES,
};
use crate::edit_mode::BlockDeviceAction;
use crate::edit_mode::{PasswordKind, PasswordUsage};

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
        self.to_string().len() + 9
    }

    /// A confirmation message to be displayed before executing the mode.
    /// When files are moved or copied the destination is displayed.
    pub fn confirmation_string(&self, destination: &str) -> String {
        match *self {
            NeedConfirmation::Copy => {
                format!("Files will be copied to {}", destination)
            }
            NeedConfirmation::Delete | NeedConfirmation::EmptyTrash => {
                "Files will be deleted permanently".to_owned()
            }
            NeedConfirmation::Move => {
                format!("Files will be moved to {}", destination)
            }
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
    SetNvimAddr,
    /// Input a password (chars a replaced by *)
    Password(PasswordKind, Option<BlockDeviceAction>, PasswordUsage),
    /// Shell command execute as is
    Shell,
    /// Mount a remote directory with sshfs
    Remote,
}

impl InputSimple {
    /// Returns a vector of static &str describing what
    /// the mode does.
    pub fn lines(&self) -> &'static [&'static str] {
        match *self {
            Self::Chmod => &CHMOD_LINES,
            Self::Filter => &FILTER_LINES,
            Self::Newdir => &NEWDIR_LINES,
            Self::Newfile => &NEWFILE_LINES,
            Self::Password(PasswordKind::SUDO, _, _) => &PASSWORD_LINES_SUDO,
            Self::Password(PasswordKind::CRYPTSETUP, _, _) => &PASSWORD_LINES_DEVICE,
            Self::RegexMatch => &REGEX_LINES,
            Self::Rename => &RENAME_LINES,
            Self::SetNvimAddr => &NVIM_ADDRESS_LINES,
            Self::Shell => &SHELL_LINES,
            Self::Sort => &SORT_LINES,
            Self::Remote => &REMOTE_LINES,
        }
    }
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
    /// Removable devices
    RemovableDevices,
    /// Manipulate an iso file to mount it
    Marks(MarkAction),
    /// Pick a compression method
    Compress,
    /// Bulk rename, new files, new directories
    Bulk,
    /// Shell menu applications. Start a new shell with this application.
    ShellMenu,
    /// Cli info
    CliInfo,
}

/// Different mode in which the application can be.
/// It dictates the reaction to event and what to display.
#[derive(Clone, Copy)]
pub enum EditMode {
    InputCompleted(InputCompleted),
    /// Select a target and navigate to it
    Navigate(Navigate),
    /// Confirmation is required before modification is made to existing files :
    /// delete, move, copy
    NeedConfirmation(NeedConfirmation),
    /// Preview a file content
    InputSimple(InputSimple),
    /// No action is currently performed
    Nothing,
}

impl fmt::Display for EditMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            EditMode::InputSimple(InputSimple::Rename) => write!(f, "Rename:  "),
            EditMode::InputSimple(InputSimple::Chmod) => write!(f, "Chmod:   "),
            EditMode::InputSimple(InputSimple::Newfile) => write!(f, "Newfile: "),
            EditMode::InputSimple(InputSimple::Newdir) => write!(f, "Newdir:  "),
            EditMode::InputSimple(InputSimple::RegexMatch) => write!(f, "Regex:   "),
            EditMode::InputSimple(InputSimple::SetNvimAddr) => write!(f, "Neovim:  "),
            EditMode::InputSimple(InputSimple::Shell) => write!(f, "Shell:   "),
            EditMode::InputSimple(InputSimple::Sort) => {
                write!(f, "Sort: Kind Name Modif Size Ext Rev :")
            }
            EditMode::InputSimple(InputSimple::Filter) => write!(f, "Filter:  "),
            EditMode::InputSimple(InputSimple::Password(password_kind, _, _)) => {
                write!(f, "{password_kind}")
            }
            EditMode::InputSimple(InputSimple::Remote) => write!(f, "Remote:  "),

            EditMode::InputCompleted(InputCompleted::Exec) => write!(f, "Exec:    "),
            EditMode::InputCompleted(InputCompleted::Goto) => write!(f, "Goto  :  "),
            EditMode::InputCompleted(InputCompleted::Search) => write!(f, "Search:  "),
            EditMode::InputCompleted(InputCompleted::Nothing) => write!(f, "Nothing:  "),
            EditMode::InputCompleted(InputCompleted::Command) => write!(f, "Command:  "),
            EditMode::Navigate(Navigate::Marks(_)) => write!(f, "Marks jump:"),
            EditMode::Navigate(Navigate::Jump) => write!(
                f,
                "Flagged files: <Enter> go to file -- <SPC> remove flag -- <u> unflag all -- <x> delete -- <X> trash"
            ),
            EditMode::Navigate(Navigate::History) => write!(f, "History :"),
            EditMode::Navigate(Navigate::Shortcut) => write!(f, "Shortcut :"),
            EditMode::Navigate(Navigate::Trash) => write!(f, "Trash :"),
            EditMode::Navigate(Navigate::ShellMenu) => {
                write!(f, "Start a new shell running a command:")
            }
            EditMode::Navigate(Navigate::Bulk) => {
                write!(f, "Bulk: rename flagged files or create new files")
            }
            EditMode::Navigate(Navigate::Compress) => write!(f, "Compress :"),
            EditMode::Navigate(Navigate::EncryptedDrive) => {
                write!(f, "Encrypted devices :")
            }
            EditMode::Navigate(Navigate::RemovableDevices) => {
                write!(f, "Removable devices :")
            }
            EditMode::Navigate(Navigate::CliInfo) => write!(f, "Display infos :"),
            EditMode::NeedConfirmation(_) => write!(f, "Y/N   :"),
            EditMode::Nothing => write!(f, ""),
        }
    }
}

#[derive(Default)]
pub enum DisplayMode {
    #[default]
    /// Display the files like `ls -lh` does
    Normal,
    /// Display files like `tree` does
    Tree,
    /// Preview a file or directory
    Preview,
}

impl DisplayMode {
    /// True if the mode requires a view refresh when left.
    /// Most modes don't, since they don't display their content in the first window.
    /// content. But `Mode::Preview` does, since it uses the main window.
    pub fn refresh_required(&self) -> bool {
        matches!(*self, DisplayMode::Preview)
    }
}
