use strfmt::strfmt;

use crate::fm_error::FmResult;
use crate::keybindings::Bindings;

/// Help message to be displayed when help key is pressed.
/// Default help key is `'h'`.

static HELP_TO_FORMAT: &str = "
{Quit}:      quit
{Help}:      help

- Navigation -
{MoveLeft}:           cd to parent directory 
{MoveRight}:          cd to child directory
{MoveUp}:             one line up  
{MoveDown}:           one line down
{KeyHome}:           go to first line
{End}:            go to last line
{PageUp}:         10 lines up
{PageDown}:       10 lines down
{Tab}:            cycle tab

- Actions -
{ToggleDualPane}:      toggle dual pane - if the width is sufficiant
{TogglePreviewSecond}:       toggle a preview on the second pane
{ToggleDisplayFull}:      toggle metadata on files
{ToggleHidden}:      toggle hidden
{Shell}:      shell in current directory
{OpenFile}:      open the selected file
{NvimFilepicker}:      open in current nvim session
{NvimSetAddress}:      setup the nvim rpc address
{Preview}:      preview this file
{MediaInfo}:       display infos about a media file
{Back}:      move back to previous dir
{Home}:      move to $HOME
{MarksNew}:      mark current path
{MarksJump}:     jump to a mark
{SearchNext}:      Search next matching element
{FuzzyFind}:      fuzzy finder
{FuzzyFindLine}:      fuzzy finder for line
{RefreshView}:      refresh view
{CopyFilename}:      copy filename to clipboard
{CopyFilepath}:      copy filepath to clipboard
{GitRoot}:      move to git root
{DragNDrop}:       dragon-drop selected file
{OpenConfig}:       open the config file
{Lazygit}:      open a new terminal with lazygit
{SetWallpaper}:      set the selected file as wallpaper with nitrogen

- Action on flagged files - 
{ToggleFlag}:      toggle flag on a file 
{FlagAll}:      flag all
{ClearFlags}:      clear flags
{ReverseFlags}:      reverse flags
{Symlink}:      symlink files
{Bulk}:      bulknew files
{CopyPaste}:      copy to current dir
{CutPaste}:      move to current dir
{DeleteFile}:      delete files permanently
{TrashMoveFile}:      move to trash
{Compress}:      compress into an archive

- Trash -
{TrashOpen}:       Open the trash (enter to restore, del clear)
{TrashEmpty}:       Empty the trash

- Tree -
Navigate as usual. Most actions works as in 'normal' view.
{Tree}:      Toggle tree mode
{TreeFold}:      Fold a node
{TreeFoldAll}:       Fold every node
{TreeUnFoldAll}:      Unfold every node
 
- MODES - 
{Tree}:      TREE
{Chmod}:      CHMOD 
{Exec}:      EXEC 
{NewDir}:      NEWDIR 
{NewFile}:      NEWFILE
{Rename}:      RENAME
{Goto}:      GOTO
{RegexMatch}:      REGEXMATCH
{Jump}:      JUMP
{Sort}:      SORT
{History}:      HISTORY
{Shortcut}:      SHORTCUT
{EncryptedDrive}:      ENCRYPTED DRIVE
    (m: open & mount,  u: unmount & close)
{Search}:      SEARCH
{Command}:      COMMAND
{Filter}:      FILTER 
    (by name \"n name\", by ext \"e ext\", only directories d or all for reset)
{Enter}:  Execute mode then NORMAL
{ModeNormal}:    NORMAL

- MOC -
Control MOC from your TUI
{MocpAddToPlayList}:          Add a file or folder to the playlist
{MocpPrevious}         Previous song
{MocpTogglePause}:        Toggle play/pause. Start MOC if needed
{MocpNext}        Next song
";

/// Holds the help string, formated with current keybindings.
pub struct Help {
    /// The help string, formated with current keybindings.
    pub help: String,
}

impl Help {
    /// Creates an Help instance from keybindings.
    /// If multiple keybindings are bound to the same action, the last one
    /// is displayed.
    pub fn from_keybindings(binds: &Bindings) -> FmResult<Self> {
        let help = strfmt(HELP_TO_FORMAT, &binds.keybind_reversed())?;
        Ok(Self { help })
    }
}
