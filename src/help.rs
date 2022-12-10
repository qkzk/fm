use log::info;
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
←:      cd to parent directory 
→:      cd to child directory
↑:      one line up  
↓:      one line down
Home:   go to first line
End:    go to last line
PgUp:   10 lines up
PgDown: 10 lines down
Tab:    Cycle tab

{ToggleHidden}:      toggle hidden
{Shell}:      shell in current directory
{OpenFile}:      open this file
{NvimFilepicker}:      open in current nvim session
{Preview}:      preview this file
Ctrl+e: toggle details on files
Ctrl+f: fuzzy finder
Ctrl+r: refresh view
Ctrl+c: copy filename to clipboard
Ctrl+p: copy filepath to clipboard
Ctrl+x: decompress selected file
Alt+d:  dragon-drop selected file
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
    Enter:  Execute mode then NORMAL
    Esc:    NORMAL
    Ctrl+q: NORMAL
"
        .to_owned();
        let hm = binds.keybind_reversed();
        info!("binds to hashmap {:?}", hm);
        let help = strfmt(&help_to_format, &binds.keybind_reversed())?;
        Ok(Self { help })
    }
}
