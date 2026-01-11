use std::fmt;

use anyhow::Result;

use crate::app::Status;
use crate::common::{
    UtfWidth, CHMOD_LINES, CLOUD_NEWDIR_LINES, FILTER_LINES, NEWDIR_LINES, NEWFILE_LINES,
    NVIM_ADDRESS_LINES, PASSWORD_LINES_DEVICE, PASSWORD_LINES_SUDO, REGEX_LINES, REMOTE_LINES,
    RENAME_LINES, SHELL_LINES, SORT_LINES,
};
use crate::event::EventAction;
use crate::modes::InputCompleted;
use crate::modes::MountAction;
use crate::modes::{PasswordKind, PasswordUsage};

/// Different kind of mark actions.
/// Either we jump to an existing mark or we save current path to a mark.
/// In both case, we'll have to listen to the next char typed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkAction {
    /// Jump to a selected mark (ie a path associated to a char)
    Jump,
    /// Creates a new mark (a path associated to a char)
    New,
}

/// Different kind of last edition command received requiring a confirmation.
/// Copy, move and delete require a confirmation to prevent big mistakes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeedConfirmation {
    /// Copy flagged files
    Copy,
    /// Delete flagged files
    Delete,
    /// Move flagged files
    Move,
    /// Empty Trash
    EmptyTrash,
    /// Bulk
    BulkAction,
    /// Delete cloud files
    DeleteCloud,
}

impl NeedConfirmation {
    /// A confirmation message to be displayed before executing the mode.
    /// When files are moved or copied the destination is displayed.
    #[must_use]
    pub fn confirmation_string(&self, destination: &str) -> String {
        match *self {
            Self::Copy => {
                format!("Files will be copied to {destination}")
            }
            Self::Delete | Self::EmptyTrash => "Files will be deleted permanently".to_owned(),
            Self::Move => {
                format!("Files will be moved to {destination}")
            }
            Self::BulkAction => "Those files will be renamed or created :".to_owned(),
            Self::DeleteCloud => "Remote Files will be deleted permanently".to_owned(),
        }
    }

    pub fn use_flagged_files(&self) -> bool {
        matches!(self, Self::Copy | Self::Move | Self::Delete)
    }
}

impl CursorOffset for NeedConfirmation {
    /// Offset before the cursor.
    /// Since we ask the user confirmation, we need to know how much space
    /// is needed.
    #[must_use]
    fn cursor_offset(&self) -> u16 {
        self.to_string().utf_width_u16() + 9
    }
}

impl Leave for NeedConfirmation {
    fn must_refresh(&self) -> bool {
        true
    }

    fn must_reset_mode(&self) -> bool {
        true
    }
}

impl std::fmt::Display for NeedConfirmation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Delete => write!(f, "Delete files :"),
            Self::DeleteCloud => write!(f, "Delete files :"),
            Self::Move => write!(f, "Move files here :"),
            Self::Copy => write!(f, "Copy files here :"),
            Self::EmptyTrash => write!(f, "Empty the trash ?"),
            Self::BulkAction => write!(f, "Bulk :"),
        }
    }
}

/// Different modes in which the user is expeted to type something.
/// It may be a new filename, a mode (aka an octal permission),
/// the name of a new file, of a new directory,
/// A regex to match all files in current directory,
/// a kind of sort, a mark name, a new mark or a filter.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    Password(Option<MountAction>, PasswordUsage),
    /// Shell command execute as is
    ShellCommand,
    /// Mount a remote directory with sshfs
    Remote,
    /// Create a new file in the current cloud
    CloudNewdir,
}

impl fmt::Display for InputSimple {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Rename => write!(f, "Rename:  "),
            Self::Chmod => write!(f, "Chmod:   "),
            Self::Newfile => write!(f, "Newfile: "),
            Self::Newdir => write!(f, "Newdir:  "),
            Self::RegexMatch => write!(f, "Regex:   "),
            Self::SetNvimAddr => write!(f, "Neovim:  "),
            Self::CloudNewdir => write!(f, "Newdir:  "),
            Self::ShellCommand => write!(f, "Shell:   "),
            Self::Sort => {
                write!(f, "Sort: ")
            }
            Self::Filter => write!(f, "Filter:  "),
            Self::Password(_, PasswordUsage::CRYPTSETUP(password_kind)) => {
                write!(f, "{password_kind}")
            }
            Self::Password(_, _) => write!(f, " sudo: "),
            Self::Remote => write!(f, "Remote:  "),
        }
    }
}

impl InputSimple {
    const EDIT_BOX_OFFSET: u16 = 11;
    const SORT_CURSOR_OFFSET: u16 = 8;
    const PASSWORD_CURSOR_OFFSET: u16 = 9;

    /// Returns a vector of static &str describing what
    /// the mode does.
    #[must_use]
    pub const fn lines(&self) -> &'static [&'static str] {
        match *self {
            Self::Chmod => &CHMOD_LINES,
            Self::Filter => &FILTER_LINES,
            Self::Newdir => &NEWDIR_LINES,
            Self::Newfile => &NEWFILE_LINES,
            Self::Password(_, PasswordUsage::CRYPTSETUP(PasswordKind::SUDO)) => {
                &PASSWORD_LINES_SUDO
            }
            Self::Password(_, PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP)) => {
                &PASSWORD_LINES_DEVICE
            }
            Self::Password(_, _) => &PASSWORD_LINES_SUDO,
            Self::RegexMatch => &REGEX_LINES,
            Self::Rename => &RENAME_LINES,
            Self::SetNvimAddr => &NVIM_ADDRESS_LINES,
            Self::ShellCommand => &SHELL_LINES,
            Self::Sort => &SORT_LINES,
            Self::Remote => &REMOTE_LINES,
            Self::CloudNewdir => &CLOUD_NEWDIR_LINES,
        }
    }
}

/// Used to know how many cells are used before the cursor is drawned it the input line.
pub trait CursorOffset {
    fn cursor_offset(&self) -> u16;
}

impl CursorOffset for InputSimple {
    fn cursor_offset(&self) -> u16 {
        match *self {
            Self::Sort => Self::SORT_CURSOR_OFFSET,
            Self::Password(_, _) => Self::PASSWORD_CURSOR_OFFSET,
            _ => Self::EDIT_BOX_OFFSET,
        }
    }
}

impl Leave for InputSimple {
    fn must_refresh(&self) -> bool {
        !matches!(
            self,
            Self::ShellCommand | Self::Filter | Self::Password(_, _) | Self::Sort
        )
    }

    fn must_reset_mode(&self) -> bool {
        !matches!(self, Self::ShellCommand | Self::Password(_, _))
    }
}

/// Different modes in which we display a bunch of possible actions.
/// In all those mode we can select an action and execute it.
/// For some of them, it's just moving there, for some it acts on some file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Navigate {
    /// Navigate back to a visited path
    History,
    /// Navigate to a predefined shortcut
    Shortcut,
    /// Manipulate trash files
    Trash,
    /// Edit a mark or cd to it
    Marks(MarkAction),
    /// Edit a temporary mark or cd to it
    TempMarks(MarkAction),
    /// See mount points, mount, unmount partions
    Mount,
    /// Pick a compression method
    Compress,
    /// Shell menu applications. Start a new shell with this application.
    TuiApplication,
    /// Cli info
    CliApplication,
    /// Context menu
    Context,
    /// Cloud menu
    Cloud,
    /// Picker menu
    Picker,
    /// Flagged files
    Flagged,
}

impl fmt::Display for Navigate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::Marks(_) => write!(f, "Marks jump:"),
            Self::TempMarks(_) => write!(f, "Temp marks jump:"),
            Self::History => write!(f, "History :"),
            Self::Shortcut => write!(f, "Shortcut :"),
            Self::Trash => write!(f, "Trash :"),
            Self::TuiApplication => {
                write!(f, "Start a new shell running a command:")
            }
            Self::Compress => write!(f, "Compress :"),
            Self::Mount => write!(f, "Mount :"),
            Self::CliApplication => write!(f, "Display infos :"),
            Self::Context => write!(f, "Context"),
            Self::Cloud => write!(f, "Cloud"),
            Self::Picker => write!(f, "Picker"),
            Self::Flagged => write!(f, "Flagged"),
        }
    }
}

impl CursorOffset for Navigate {
    #[inline]
    #[must_use]
    fn cursor_offset(&self) -> u16 {
        0
    }
}

impl Leave for Navigate {
    fn must_refresh(&self) -> bool {
        !matches!(self, Self::CliApplication | Self::Context)
    }

    fn must_reset_mode(&self) -> bool {
        !matches!(self, Self::CliApplication | Self::Context)
    }
}

impl Navigate {
    /// True if the draw_menu trait can be called directly to display this mode
    pub fn simple_draw_menu(&self) -> bool {
        matches!(
            self,
            Self::Compress
                | Self::Shortcut
                | Self::TuiApplication
                | Self::CliApplication
                | Self::Marks(_)
                | Self::Mount
        )
    }
}

/// Different "menu" mode in which the application can be.
/// It dictates the reaction to event and is displayed in the bottom window.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Menu {
    /// Do something that may be completed
    /// Completion may come from :
    /// - executable in $PATH,
    /// - current directory or tree,
    /// - directory in your file system,
    /// - known actions. See [`crate::event::EventAction`],
    InputCompleted(InputCompleted),
    /// Do something that need typing :
    /// - renaming a file or directory,
    /// - creating a file or directory,
    /// - typing a password (won't be displayed, will be dropped ASAP)
    InputSimple(InputSimple),
    /// Select something in a list and act on it
    Navigate(Navigate),
    /// Confirmation is required before modification is made to existing files :
    /// delete, move, copy
    NeedConfirmation(NeedConfirmation),
    /// No action is currently performed
    Nothing,
}

impl fmt::Display for Menu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::InputCompleted(input_completed) => input_completed.fmt(f),
            Self::InputSimple(input_simple) => input_simple.fmt(f),
            Self::Navigate(navigate) => navigate.fmt(f),
            Self::NeedConfirmation(need_confirmation) => need_confirmation.fmt(f),
            Self::Nothing => write!(f, ""),
        }
    }
}

impl Menu {
    /// Does this mode requires a cursor ?
    pub fn show_cursor(&self) -> bool {
        self.cursor_offset() != 0
    }

    pub fn binds_per_mode(&self) -> &'static str {
        match self {
            Self::InputCompleted(_) => "Tab: completion. shift+⬆️, shift+⬇️: previous entries, shift+⬅️: erase line. Enter: validate",
            Self::InputSimple(InputSimple::Filter) => "Enter reset the filters",
            Self::InputSimple(InputSimple::Sort ) => "Enter reset the sort",
            Self::InputSimple(_) => "shift+⬆️, shift+⬇️: previous entries, shift+⬅️: erase line. Enter: validate",
            Self::Navigate(Navigate::Marks(MarkAction::Jump)) => "Type the mark letter to jump there. up, down to navigate, ENTER to select an element",
            Self::Navigate(Navigate::Marks(MarkAction::New)) => "Type the mark set a mark here. up, down to navigate, ENTER to select an element",
            Self::Navigate(Navigate::TempMarks(MarkAction::New)) => "Type the mark set a mark here. up, down to navigate, ENTER to select an element",
            Self::Navigate(Navigate::Cloud) => "l: leave drive, arrows: navigation, Enter: enter dir / download file, d: new dir, x: delete selected, u: upload local file",
            Self::Navigate(Navigate::Flagged) => "Up, Down: navigate, Enter / j: jump to this file, x: remove from flagged, u: clear",
            Self::Navigate(Navigate::Trash) => "Up, Down: navigate.",
            Self::Navigate(Navigate::Mount) => "m: mount, u: umount, e: eject (removable), g/ENTER: go to",
            Self::Navigate(_) => "up, down to navigate, Enter to select an element",
            Self::NeedConfirmation(_) => "",
            _ => "",
        }
    }

    /// True if the edit mode is "Nothing" aka no menu is opened in this tab.
    pub fn is_nothing(&self) -> bool {
        matches!(self, Self::Nothing)
    }

    pub fn is_navigate(&self) -> bool {
        matches!(self, Self::Navigate(_))
    }

    /// True iff the menu requires an input aka InputSimple or InputCompleted.
    pub fn is_input(&self) -> bool {
        matches!(self, Self::InputCompleted(_) | Self::InputSimple(_))
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::InputCompleted(_))
    }
}

impl CursorOffset for Menu {
    /// Constant offset for the cursor.
    /// In any mode, we display the mode used and then the cursor if needed.
    #[inline]
    #[must_use]
    fn cursor_offset(&self) -> u16 {
        match self {
            Self::InputCompleted(input_completed) => input_completed.cursor_offset(),
            Self::InputSimple(input_simple) => input_simple.cursor_offset(),
            Self::Navigate(navigate) => navigate.cursor_offset(),
            Self::NeedConfirmation(confirmed_action) => confirmed_action.cursor_offset(),
            Self::Nothing => 0,
        }
    }
}

impl Leave for Menu {
    fn must_refresh(&self) -> bool {
        match self {
            Self::InputCompleted(input_completed) => input_completed.must_refresh(),
            Self::InputSimple(input_simple) => input_simple.must_refresh(),
            Self::Navigate(navigate) => navigate.must_refresh(),
            Self::NeedConfirmation(need_confirmation) => need_confirmation.must_refresh(),
            Self::Nothing => true,
        }
    }

    fn must_reset_mode(&self) -> bool {
        match self {
            Self::InputCompleted(input_completed) => input_completed.must_reset_mode(),
            Self::InputSimple(input_simple) => input_simple.must_reset_mode(),
            Self::Navigate(navigate) => navigate.must_reset_mode(),
            Self::NeedConfirmation(need_confirmation) => need_confirmation.must_reset_mode(),
            Self::Nothing => true,
        }
    }
}

/// Trait which should be implemented for every edit mode.
/// It says if leaving this mode should be followed with a reset of the display & file content,
/// and if we have to reset the edit mode.
pub trait Leave {
    /// Should the file content & window be refreshed when leaving this mode?
    fn must_refresh(&self) -> bool;
    /// Should the edit mode be reset to Nothing when leaving this mode ?
    fn must_reset_mode(&self) -> bool;
}

pub trait ReEnterMenu {
    fn reenter(&self, status: &mut Status) -> Result<()>;
}

impl ReEnterMenu for Menu {
    #[rustfmt::skip]
    fn reenter(&self, status: &mut Status) -> Result<()> {
        match self {
            Self::InputCompleted(InputCompleted::Cd)                => EventAction::cd(status),
            Self::InputCompleted(InputCompleted::Search)            => EventAction::search(status),
            Self::InputCompleted(InputCompleted::Exec)              => EventAction::exec(status),
            Self::InputCompleted(InputCompleted::Action)            => EventAction::action(status),
            Self::InputSimple(InputSimple::Rename)                  => EventAction::rename(status),
            Self::InputSimple(InputSimple::Chmod)                   => EventAction::chmod(status),
            Self::InputSimple(InputSimple::Newfile)                 => EventAction::new_file(status),
            Self::InputSimple(InputSimple::Newdir)                  => EventAction::new_dir(status),
            Self::InputSimple(InputSimple::RegexMatch)              => EventAction::regex_match(status),
            Self::InputSimple(InputSimple::Sort)                    => EventAction::sort(status),
            Self::InputSimple(InputSimple::Filter)                  => EventAction::filter(status),
            Self::InputSimple(InputSimple::SetNvimAddr)             => EventAction::set_nvim_server(status),
            Self::InputSimple(InputSimple::Password(_mount_action, _usage)) => unreachable!("Can't pick a password, those aren't saved."),
            Self::InputSimple(InputSimple::ShellCommand)            => EventAction::shell_command(status),
            Self::InputSimple(InputSimple::Remote)                  => EventAction::remote_mount(status),
            Self::InputSimple(InputSimple::CloudNewdir)             => EventAction::cloud_enter_newdir_mode(status),
            Self::Navigate(Navigate::History)                       => EventAction::history(status),
            Self::Navigate(Navigate::Shortcut)                      => EventAction::shortcut(status),
            Self::Navigate(Navigate::Trash)                         => EventAction::trash_open(status),
            Self::Navigate(Navigate::Marks(markaction)) => match markaction {
                MarkAction::Jump => EventAction::marks_jump(status),
                MarkAction::New => EventAction::marks_new(status),
            },
            Self::Navigate(Navigate::TempMarks(markaction)) => match markaction {
                MarkAction::Jump => EventAction::temp_marks_jump(status),
                MarkAction::New => EventAction::temp_marks_new(status),
            },
            Self::Navigate(Navigate::Mount)                         => EventAction::mount(status),
            Self::Navigate(Navigate::Picker)                        => unreachable!("Can't reenter picker from itself"),
            Self::Navigate(Navigate::Compress)                      => EventAction::compress(status),
            Self::Navigate(Navigate::TuiApplication)                => EventAction::tui_menu(status),
            Self::Navigate(Navigate::CliApplication)                => EventAction::cli_menu(status),
            Self::Navigate(Navigate::Context)                       => EventAction::context(status),
            Self::Navigate(Navigate::Cloud)                         => EventAction::cloud_drive(status),
            Self::Navigate(Navigate::Flagged)                       => EventAction::display_flagged(status),
            Self::NeedConfirmation(NeedConfirmation::Copy)          => EventAction::copy_paste(status),
            Self::NeedConfirmation(NeedConfirmation::Delete)        => EventAction::delete_file(status),
            Self::NeedConfirmation(NeedConfirmation::DeleteCloud)   => EventAction::cloud_enter_delete_mode(status),
            Self::NeedConfirmation(NeedConfirmation::Move)          => EventAction::cut_paste(status),
            Self::NeedConfirmation(NeedConfirmation::BulkAction)    => EventAction::bulk(status),
            Self::NeedConfirmation(NeedConfirmation::EmptyTrash)    => EventAction::trash_empty(status),
            Self::Nothing                                           => Ok(()),
        }
    }
}

/// What kind of content is displayed in the main window of this tab.
/// Directory (all files of a directory), Tree (all files and children up to a certain depth),
/// preview of a content (file, command output...) or fuzzy finder of file.
#[derive(Default, PartialEq, Clone, Copy)]
pub enum Display {
    #[default]
    /// Display the files like `ls -lh` does
    Directory,
    /// Display files like `tree` does
    Tree,
    /// Preview a file or directory
    Preview,
    /// Fuzzy finder of something
    Fuzzy,
}

impl Display {
    fn is(&self, other: Self) -> bool {
        self == &other
    }

    pub fn is_tree(&self) -> bool {
        self.is(Self::Tree)
    }

    pub fn is_preview(&self) -> bool {
        self.is(Self::Preview)
    }

    pub fn is_fuzzy(&self) -> bool {
        self.is(Self::Fuzzy)
    }

    pub fn is_directory(&self) -> bool {
        self.is(Self::Directory)
    }
}
