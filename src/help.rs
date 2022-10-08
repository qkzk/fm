/// Help message to be displayed when help key is pressed.
/// Default help key is `'h'`.
pub static HELP_LINES: &str = "
Default key bindings:

q:      quit
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
o:      xdg-open this file
i:      open in current nvim session
P:      preview this file

- Action on flagged files - 
    space:  toggle flag on a file 
    *:      flag all
    u:      clear flags
    v:      reverse flags
    c:      copy to current dir
    p:      move to current dir
    x:      delete flagged files
    S:      symlink flagged files

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
    Enter:  Execute mode then NORMAL
    Esc:    NORMAL
";
