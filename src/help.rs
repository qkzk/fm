use strfmt::strfmt;

use crate::fm_error::FmResult;
use crate::keybindings::Bindings;

/// Help message to be displayed when help key is pressed.
/// Default help key is `'h'`.

pub struct Help {
    pub help: String,
}

impl Help {
    pub fn from_keybindings(binds: &Bindings) -> FmResult<Self> {
        let help_to_format = "
{Quit}:      quit
{Help}:      help

- Navigation -
{MoveLeft}:      cd to parent directory 
{MoveRight}:      cd to child directory
{MoveUp}:      one line up  
{MoveDown}:      one line down
{KeyHome}:   go to first line
{End}:    go to last line
{PageUp}:   10 lines up
{PageDown}: 10 lines down
{Tab}:    Cycle tab

{ToggleHidden}:      toggle hidden
{Shell}:      shell in current directory
{OpenFile}:      open this file
{NvimFilepicker}:      open in current nvim session
{Preview}:      preview this file
{DisplayFull}: toggle details on files
{FuzzyFind}: fuzzy finder
{RefreshView}: refresh view
{CopyFilename}: copy filename to clipboard
{CopyFilepath}: copy filepath to clipboard
{Decompress}: decompress selected file
{DragNDrop}:  dragon-drop selected file
{MarksNew}:      Mark current path
{MarksJump}:      Jump to a mark
{Back}:      Move back to previous dir
{Home}:      Move to $HOME

- Action on flagged files - 
    space:  toggle flag on a file 
{FlagAll}:      flag all
{ClearFlags}:      clear flags
{ReverseFlags}:      reverse flags
{CopyPaste}:      copy to current dir
{CutPaste}:      move to current dir
{DeleteFile}:      delete files
{Symlink}:      symlink files
{Bulkrename}:      Bulkrename files

- MODES - 
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
    {Search}:      SEARCH
    {Filter}:      FILTER 
        (by name \"n name\", by ext \"e ext\", only directories d or all for reset)
    {Enter}:  Execute mode then NORMAL
    {ModeNormal}:    NORMAL
"
        .to_owned();
        let help = strfmt(&help_to_format, &binds.keybind_reversed())?;
        Ok(Self { help })
    }
}
