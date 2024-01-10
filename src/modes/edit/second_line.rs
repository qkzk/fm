use crate::modes::{Edit, InputCompleted, InputSimple, MarkAction, Navigate, NeedConfirmation};

pub trait SecondLine {
    /// Line describing the mode and its custom keybinds
    fn second_line(&self) -> &'static str;
}

impl SecondLine for Edit {
    fn second_line(&self) -> &'static str {
        match self {
            Self::Navigate(navigate) => navigate.second_line(),
            Self::InputSimple(input_simple) => input_simple.second_line(),
            Self::NeedConfirmation(need_confirmation) => need_confirmation.second_line(),
            Self::InputCompleted(input_completed) => input_completed.second_line(),
            Self::Nothing => "",
        }
    }
}

impl SecondLine for Navigate {
    fn second_line(&self) -> &'static str {
        match self {
            Self::Jump => "Pick a destination",
            Self::Trash => "",
            Self::History => "Pick a destination",
            Self::Shortcut => "Pick a destination",
            Self::Compress => "Archive and compress the flagged files using selected algorithm.",
            Self::BulkMenu => "Pick an action",
            Self::Marks(mark_action) => mark_action.second_line(),
            Self::Context => "Pick an action",
            Self::EncryptedDrive => "m: mount   --   u: unmount   --   g: go to mount point",
            Self::TuiApplication => "Pick a command",
            Self::CliApplication => "Pick a command",
            Self::RemovableDevices => "",
        }
    }
}

impl SecondLine for MarkAction {
    fn second_line(&self) -> &'static str {
        match self {
            Self::Jump => "Select a mark to go to or type its char",
            Self::New => {
                "Select a mark or type its char to update it. <Backspace> erase the selected mark"
            }
        }
    }
}

impl SecondLine for InputCompleted {
    fn second_line(&self) -> &'static str {
        match self {
            Self::Cd => "Type your destination",
            Self::Search => "Type a pattern to search",
            Self::Exec => "Type a program",
            Self::Action => "Type an fm action",
        }
    }
}

impl SecondLine for InputSimple {
    fn second_line(&self) -> &'static str {
        ""
    }
}

impl SecondLine for NeedConfirmation {
    fn second_line(&self) -> &'static str {
        ""
    }
}
