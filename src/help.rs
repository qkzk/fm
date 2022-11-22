/// Help message to be displayed when help key is pressed.
/// Default help key is `'h'`.
pub static HELP_LINES: &str = "
Default key bindings:

q:      quit
Atl+q:  quit and print selected path
h:      help

- Navigation -
←:      cd to parent directory 
→:      cd to child directory
↑:      one line up  
↓:      one line down
Home:   go to first line
End:    go to last line
PgUp:   10 lines up
PgDown: 10 lines down

a:      toggle hidden
s:      shell in current directory
o:      open this file
i:      open in current nvim session
P:      preview this file
Ctrl+f: fuzzy finder
Ctrl+r: refresh view
Ctrl+c: copy filename to clipboard
Ctrl+p: copy filepath to clipboard
Ctrl+x: decompress selected file
M:      Mark current path
':      Jump to a mark

- Tabs -
    Tab:    Next tab
    Del:    Close the current tab
    Ins:    Insert a new tab

- Action on flagged files - 
    space:  toggle flag on a file 
    *:      flag all
    u:      clear flags
    v:      reverse flags
    c:      copy to current dir
    p:      move to current dir
    x:      delete files
    S:      symlink files
    B:      Bulkrename files

- MODES - 
    m:      CHMOD 
    e:      EXEC 
    d:      NEWDIR 
    n:      NEWFILE
    r:      RENAME
    g:      GOTO
    w:      REGEXMATCH
    j:      JUMP
    O:      SORT
    H:      HISTORY
    G:      SHORTCUT
    /:      SEARCH
    f:      FILTER 
        (by name \"n name\", by ext \"e ext\", only directories d or all for reset)
    Enter:  Execute mode then NORMAL
    Esc:    NORMAL
    Ctrl+q: NORMAL
";
