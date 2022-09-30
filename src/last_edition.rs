use std::fmt;

#[derive(Debug)]
pub enum LastEdition {
    Nothing,
    Delete,
    CutPaste,
    CopyPaste,
}

impl LastEdition {
    pub fn offset(&self) -> usize {
        match *self {
            Self::Nothing => 0,
            Self::CopyPaste => 37,
            Self::Delete => 31,
            Self::CutPaste => 29,
        }
    }
}

impl std::fmt::Display for LastEdition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            LastEdition::Nothing => write!(f, "Nothing to confirm"),
            LastEdition::Delete => write!(f, "Delete files :"),
            LastEdition::CutPaste => write!(f, "Move files :"),
            LastEdition::CopyPaste => write!(f, "Copy & Paste files :"),
        }
    }
}
