use crate::app::Status;
use crate::modes::{
    InputCompleted, InputSimple, MarkAction, Menu, Navigate, NeedConfirmation, PasswordKind,
    PasswordUsage,
};

/// Used by different kind of menu to display informations about the current menu.
/// Most of the time it's a few lines describing the menu actions.
pub trait LineDisplay {
    /// Returns a displayable representation of the object as a vector of `String`s
    fn line_display(&self, status: &Status) -> Vec<String>;
}

impl LineDisplay for Menu {
    fn line_display(&self, status: &Status) -> Vec<String> {
        match self {
            Self::Navigate(mode) => mode.line_display(status),
            Self::InputSimple(mode) => mode.line_display(status),
            Self::InputCompleted(mode) => mode.line_display(status),
            Self::NeedConfirmation(mode) => mode.line_display(status),
            Self::Nothing => vec![],
        }
    }
}

impl LineDisplay for NeedConfirmation {
    fn line_display(&self, _status: &Status) -> Vec<String> {
        vec![format!("{self}"), " (y/n)".to_owned()]
    }
}

impl LineDisplay for Navigate {
    fn line_display(&self, _status: &Status) -> Vec<String> {
        match self {
            Self::Marks(MarkAction::Jump) => {
                vec!["Jump to...".to_owned()]
            }
            Self::Marks(MarkAction::New) => {
                vec!["Save mark...".to_owned()]
            }
            _ => {
                vec![Menu::Navigate(*self).to_string()]
            }
        }
    }
}

impl LineDisplay for InputCompleted {
    fn line_display(&self, status: &Status) -> Vec<String> {
        let tab = status.current_tab();
        let mut completion_strings = vec![tab.menu_mode.to_string(), status.menu.input.string()];
        if let Some(completion) = status
            .menu
            .completion
            .complete_input_string(&status.menu.input.string())
        {
            completion_strings.push(completion.to_owned());
        }
        if matches!(*self, Self::Exec) {
            for path in &status.menu.flagged.content {
                completion_strings.push(format!(" {path}", path = path.display()));
            }
        }
        completion_strings
    }
}

impl LineDisplay for InputSimple {
    fn line_display(&self, status: &Status) -> Vec<String> {
        match self {
            Self::Password(_, PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP)) => {
                vec![
                    PasswordKind::CRYPTSETUP.to_string(),
                    status.menu.input.password(),
                ]
            }
            Self::Password(..) => {
                vec![PasswordKind::SUDO.to_string(), status.menu.input.password()]
            }
            _ => {
                vec![
                    Menu::InputSimple(*self).to_string(),
                    status.menu.input.string(),
                ]
            }
        }
    }
}
