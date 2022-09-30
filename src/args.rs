use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
/// FM : dired like file manager{n}
/// Default key bindings:{n}
///{n}
/// q:      quit{n}
/// h:      help{n}
///{n}
/// - Navigation -{n}
/// ←:      cd to parent directory{n}
/// →:      cd to child directory{n}
/// ↑:      one line up  {n}
/// ↓:      one line down{n}
/// Home:   go to first line{n}
/// End:    go to last line{n}
/// PgUp:   10 lines up{n}
/// PgDown: 10 lines down{n}
///{n}
/// a:      toggle hidden{n}
/// s:      shell in current directory{n}
/// o:      xdg-open this file{n}
/// i:      open with current NVIM session{n}
///{n}
/// - Action on flagged files -{n}
///     space:  toggle flag on a file{n}
///     *:      flag all{n}
///     u:      clear flags{n}
///     v:      reverse flags{n}
///     c:      copy to current dir{n}
///     p:      move to current dir{n}
///     x:      delete flagged files{n}
///{n}
/// - MODES -{n}
///     m:      CHMOD{n}
///     e:      EXEC{n}
///     d:      NEWDIR{n}
///     n:      NEWFILE{n}
///     r:      RENAME{n}
///     g:      GOTO{n}
///     w:      REGEXMATCH{n}
///     j:      JUMP{n}
///     O:      SORT{n}
///     Enter:  Execute mode then NORMAL{n}
///     Esc:    NORMAL{n}
pub struct Args {
    /// Starting path
    #[arg(short, long, default_value_t = String::from("."))]
    pub path: String,

    /// Display all files
    #[arg(short, long, default_value_t = false)]
    pub all: bool,

    /// Nvim server
    #[arg(short, long, default_value_t = String::from(""))]
    pub server: String,
}
