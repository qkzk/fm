use std::fmt;

#[derive(Clone)]
pub enum Mode {
    Normal,
    Rename,
    Chmod,
    Newfile,
    Newdir,
    Exec,
    Help,
    Search,
    Goto,
    RegexMatch,
    Jump,
    NeedConfirmation,
    Sort,
}

impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Mode::Normal => write!(f, "Normal:  "),
            Mode::Rename => write!(f, "Rename:  "),
            Mode::Chmod => write!(f, "Chmod:   "),
            Mode::Newfile => write!(f, "Newfile: "),
            Mode::Newdir => write!(f, "Newdir:  "),
            Mode::Exec => write!(f, "Exec:    "),
            Mode::Help => write!(f, ""),
            Mode::Search => write!(f, "Search:  "),
            Mode::Goto => write!(f, "Goto  :  "),
            Mode::RegexMatch => write!(f, "Regex :  "),
            Mode::Jump => write!(f, "Jump  :  "),
            Mode::NeedConfirmation => write!(f, "Y/N   :"),
            Mode::Sort => write!(f, "(N)ame (D)ate (S)ize (E)xt : "),
        }
    }
}
