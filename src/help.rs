use strfmt::strfmt;

use crate::config::Keybindings;
use crate::fm_error::FmResult;

/// Help message to be displayed when help key is pressed.
/// Default help key is `'h'`.

pub struct Help {
    pub help: String,
}

impl Help {
    pub fn from_keybindings(keybindings: &Keybindings) -> FmResult<Self> {
        let help_to_format = "
{quit}:      quit
{help}:      help

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

{toggle_hidden}:      toggle hidden
{shell}:      shell in current directory
{open_file}:      open this file
{nvim}:      open in current nvim session
{preview}:      preview this file
Ctrl+f: fuzzy finder
Ctrl+r: refresh view
Ctrl+c: copy filename to clipboard
Ctrl+p: copy filepath to clipboard
Ctrl+x: decompress selected file
Alt+d:  dragon-drop selected file
{marks_new}:      Mark current path
{marks_jump}:      Jump to a mark
{back}:      Move back to previous dir
{home}:      Move to $HOME

- Action on flagged files - 
    space:  toggle flag on a file 
{flag_all}:      flag all
{clear_flags}:      clear flags
{reverse_flags}:      reverse flags
{copy_paste}:      copy to current dir
{cut_paste}:      move to current dir
{delete}:      delete files
{symlink}:      symlink files
{bulkrename}:      Bulkrename files

- MODES - 
    {chmod}:      CHMOD 
    {exec}:      EXEC 
    {newdir}:      NEWDIR 
    {newfile}:      NEWFILE
    {rename}:      RENAME
    {goto}:      GOTO
    {regex_match}:      REGEXMATCH
    {jump}:      JUMP
    {sort_by}:      SORT
    {history}:      HISTORY
    {shortcut}:      SHORTCUT
    {search}:      SEARCH
    {filter}:      FILTER 
        (by name \"n name\", by ext \"e ext\", only directories d or all for reset)
    Enter:  Execute mode then NORMAL
    Esc:    NORMAL
    Ctrl+q: NORMAL
"
        .to_owned();
        let help = strfmt(&help_to_format, &keybindings.to_hashmap())?;
        Ok(Self { help })
    }
}
