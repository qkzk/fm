use anyhow::Result;
use strfmt::strfmt;

use crate::keybindings::Bindings;
use crate::opener::Opener;

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
{OpenFile}:      open the selected file with :
    - default       {Default}
    - audio         {Audio}
    - images        {Bitmap}
    - office        {Office}
    - pdf, ebooks   {Readable}
    - text          {Text}
    - video         {Video}
    - vectorials    {Vectorial}
    - compressed files are decompressed
    - iso images are mounted
{NvimFilepicker}:      open in current nvim session
{NvimSetAddress}:      setup the nvim rpc address
{Preview}:      preview this file
{MediaInfo}:       display infos about a media file
{Back}:      move back to previous dir
{Home}:      move to $HOME
{GoRoot}:      move to root (/)
{GoStart}:      move to starting point
{MarksNew}:      mark current path
{MarksJump}:     jump to a mark
{SearchNext}:      search next matching element
{FuzzyFind}:      fuzzy finder
{FuzzyFindLine}:      fuzzy finder for line
{FuzzyFindHelp}:     fuzzy finder from help
{RefreshView}:      refresh view
{CopyFilename}:      copy filename to clipboard
{CopyFilepath}:      copy filepath to clipboard
{DragNDrop}:       dragon-drop selected file
{OpenConfig}:       open the config file
{SetWallpaper}:      set the selected file as wallpaper with nitrogen

- Action on flagged files - 
{ToggleFlag}:      toggle flag on a file 
{FlagAll}:      flag all
{ClearFlags}:      clear flags
{ReverseFlags}:      reverse flags
{Symlink}:      symlink to current dir
{CopyPaste}:      copy to current dir
{CutPaste}:      move to current dir
{DeleteFile}:      delete files permanently
{TrashMoveFile}:      move to trash
{Compress}:      compress into an archive
{Diff}:      display the diff of the first 2 flagged files

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
{Bulk}:      BULK
{ShellMenu}:      SHELL MENU
{Filter}:      FILTER 
    (by name \"n name\", by ext \"e ext\", only directories d or all for reset)
{Enter}:  Execute mode then NORMAL
{ResetMode}:    NORMAL

- MOC -
Control MOC from your TUI
{MocpAddToPlayList}:          MOCP: Add a file or folder to the playlist
{MocpPrevious}:        MOCP: Previous song
{MocpTogglePause}:        MOCP: Toggle play/pause.
{MocpNext}:       MOCP: Next song
{MocpGoToSong}:        MOCP: Go to currently playing song 
";

const CUSTOM_HELP: &str = "
- CUSTOM -
%s: the selected file,
%f: the flagged files,
%e: the extension of the file,
%n: the filename only,
%p: the full path of the current directory.
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
    pub fn from_keybindings(binds: &Bindings, opener: &Opener) -> Result<Self> {
        let mut strings = binds.keybind_reversed();
        let openers = opener.opener_association.as_map_of_strings();
        log::info!("{openers:?}");
        strings.extend(openers);
        let mut help = strfmt(HELP_TO_FORMAT, &strings)?;
        help = Self::complete_with_custom_binds(&binds.custom, help);
        Ok(Self { help })
    }

    fn complete_with_custom_binds(custom_binds: &Option<Vec<String>>, mut help: String) -> String {
        if let Some(customs) = &custom_binds {
            help.push_str(CUSTOM_HELP);
            for custom in customs.iter() {
                help.push_str(custom);
            }
        }
        help
    }
}
