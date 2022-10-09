use std::fmt;

/// Different kind of last edition command received.
#[derive(Debug, Clone)]
pub enum LastEdition {
    /// No edition command
    Nothing,
    /// Delete flagged files
    Delete,
    /// Move flagged files
    CutPaste,
    /// Copy flagged files
    CopyPaste,
}

impl LastEdition {
    /// Offset before the cursor.
    /// Since we ask the user confirmation, we need to know how much space
    /// is needed.
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
