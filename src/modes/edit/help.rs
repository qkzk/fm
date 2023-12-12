use std::collections::HashMap;

use anyhow::Result;
use strfmt::strfmt;

use crate::config::Bindings;
use crate::io::Opener;

/// Help message to be displayed when help key is pressed.
/// Default help key is `'h'`.

const HELP_TO_FORMAT: &str = "
{Quit:<10}:      quit
{Help:<10}:      help

- Navigation -
{MoveLeft:<10}:      cd to parent directory 
{MoveRight:<10}:      cd to child directory
{MoveUp:<10}:      one line up  
{MoveDown:<10}:      one line down
{KeyHome:<10}:      go to first line
{End:<10}:      go to last line
{PageUp:<10}:      10 lines up
{PageDown:<10}:      10 lines down
{Tab:<10}:      cycle tab

- Actions -
{ToggleDualPane:<10}:      toggle dual pane - if the width is sufficiant
{TogglePreviewSecond:<10}:      toggle a preview on the second pane
{ToggleDisplayFull:<10}:      toggle metadata on files
{ToggleHidden:<10}:      toggle hidden
{Shell:<10}:      shell in current directory
{OpenFile:<10}:      open the selected file with :
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
{NvimFilepicker:<10}:      open in current nvim session
{NvimSetAddress:<10}:      setup the nvim rpc address
{Preview:<10}:      preview this file
{Back:<10}:      move back to previous dir
{Home:<10}:      move to $HOME
{GoRoot:<10}:      move to root (/)
{GoStart:<10}:      move to starting point
{MarksNew:<10}:      mark current path
{MarksJump:<10}:      jump to a mark
{SearchNext:<10}:      search next matching element
{FuzzyFind:<10}:      fuzzy finder for file
{FuzzyFindLine:<10}:      fuzzy finder for line
{FuzzyFindHelp:<10}:      fuzzy finder from help
{RefreshView:<10}:      refresh view
{CopyFilename:<10}:      copy filename to clipboard
{CopyFilepath:<10}:      copy filepath to clipboard
{OpenConfig:<10}:      open the config file

- Action on flagged files - 
{ToggleFlag:<10}:      toggle flag on a file 
{FlagAll:<10}:      flag all
{ClearFlags:<10}:      clear flags
{ReverseFlags:<10}:      reverse flags
{Symlink:<10}:      symlink to current dir
{CopyPaste:<10}:      copy to current dir
{CutPaste:<10}:      move to current dir
{DeleteFile:<10}:      delete files permanently
{TrashMoveFile:<10}:      move to trash
{Compress:<10}:      compress into an archive

- Trash -
{TrashOpen:<10}:      Open the trash (enter to restore, del clear)
{TrashEmpty:<10}:      Empty the trash

- Tree -
Navigate as usual. Most actions works as in 'normal' view.
{Tree:<10}:      Toggle tree mode
{TreeFold:<10}:      Fold a node
{TreeFoldAll:<10}:      Fold every node
{TreeUnFoldAll:<10}:      Unfold every node
 
- MODES - 
{Tree:<10}:      TREE
{Chmod:<10}:      CHMOD 
{Exec:<10}:      EXEC 
{NewDir:<10}:      NEWDIR 
{NewFile:<10}:      NEWFILE
{Rename:<10}:      RENAME
{Goto:<10}:      GOTO
{RegexMatch:<10}:      REGEXMATCH
{Jump:<10}:      JUMP
{Sort:<10}:      SORT
{History:<10}:      HISTORY
{Shortcut:<10}:      SHORTCUT
{EncryptedDrive:<10}:      ENCRYPTED DRIVE
    (m: open & mount,  u: unmount & close, g: go there)
{RemovableDevices:<10}:      REMOVABLE MTP DEVICES
    (m: mount,  u: unmount, g: go there)
{Search:<10}:      SEARCH
{Command:<10}:      COMMAND
{Bulk:<10}:      BULK
{TuiMenu:<10}:      TUI APPS
{CliMenu:<10}:      CLI APPS
{RemoteMount:<10}:      MOUNT REMOTE PATH
{Filter:<10}:      FILTER 
    (by name \"n name\", by ext \"e ext\", only directories d or all for reset)
{Enter:<10}:      Execute mode then NORMAL
{ResetMode:<10}:      NORMAL

- MOC -
Control MOC from your TUI
{MocpAddToPlayList:<10}:      MOCP: Add selected file or folder to the playlist
{MocpPrevious:<10}:      MOCP: Previous song
{MocpTogglePause:<10}:      MOCP: Toggle play/pause.
{MocpNext:<10}:      MOCP: Next song
{MocpGoToSong:<10}:      MOCP: Go to currently playing song 
{MocpClearPlaylist:<10}:      MOCP: Clear the playlist
";

const CUSTOM_HELP: &str = "
- CUSTOM ACTIONS -
%s: the selected file,
%f: the flagged files,
%e: the extension of the file,
%n: the filename only,
%p: the full path of the current directory.
";

/// Creates the help `String` from keybindings.
/// If multiple keybindings are bound to the same action, the last one
/// is displayed.
/// If an action displayed in help isn't bound to a key, the formating won't
/// be possible. We use the default keybindings instead.
/// If it doesn't work, we return an empty string.
pub fn help_string(binds: &Bindings, opener: &Opener) -> String {
    match make_help_with_config(binds, opener) {
        Ok(help) => help,
        Err(error) => {
            crate::log_info!("Error parsing help: {error}");
            let mut help = format!(
                "Couldn't parse your keybindings: {error}.
Using default keybindings.

"
            );
            help.push_str(&make_help_with_config(&Bindings::new(), opener).unwrap_or_default());
            help
        }
    }
}

fn make_help_with_config(binds: &Bindings, opener: &Opener) -> Result<String> {
    let mut keybind_reversed = binds.keybind_reversed();
    keybind_reversed.extend(opener.association.as_map_of_strings());
    let mut help = strfmt(HELP_TO_FORMAT, &keybind_reversed)?;
    help = complete_with_custom_binds(&binds.custom, help);
    Ok(help)
}

fn complete_with_custom_binds(custom_binds: &Option<Vec<String>>, mut help: String) -> String {
    if let Some(customs) = &custom_binds {
        help.push_str(CUSTOM_HELP);
        for custom in customs {
            help.push_str(custom);
        }
    }
    help
}
