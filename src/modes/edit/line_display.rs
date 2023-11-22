use crate::app::Tab;
use crate::modes::{
    Edit, InputCompleted, InputSimple, MarkAction, Navigate, NeedConfirmation, PasswordKind,
    PasswordUsage,
};

pub trait LineDisplay {
    /// Returns a displayable representation of the object as a vector of `String`s
    fn line_display(&self, tab: &Tab) -> Vec<String>;
}

impl LineDisplay for Edit {
    fn line_display(&self, tab: &Tab) -> Vec<String> {
        match self {
            Self::Navigate(mode) => mode.line_display(tab),
            Self::InputSimple(mode) => mode.line_display(tab),
            Self::InputCompleted(mode) => mode.line_display(tab),
            Self::NeedConfirmation(mode) => mode.line_display(tab),
            Self::Nothing => vec![],
        }
    }
}

impl LineDisplay for NeedConfirmation {
    fn line_display(&self, _tab: &Tab) -> Vec<String> {
        vec![format!("{self}"), " (y/n)".to_owned()]
    }
}

impl LineDisplay for Navigate {
    fn line_display(&self, _tab: &Tab) -> Vec<String> {
        match self {
            Self::Marks(MarkAction::Jump) => {
                vec!["Jump to...".to_owned()]
            }
            Self::Marks(MarkAction::New) => {
                vec!["Save mark...".to_owned()]
            }
            _ => {
                vec![Edit::Navigate(*self).to_string()]
            }
        }
    }
}

impl LineDisplay for InputCompleted {
    fn line_display(&self, tab: &Tab) -> Vec<String> {
        let mut completion_strings = vec![tab.edit_mode.to_string(), tab.input.string()];
        if let Some(completion) = tab.completion.complete_input_string(&tab.input.string()) {
            completion_strings.push(completion.to_owned());
        }
        if matches!(*self, Self::Exec) {
            if let Ok(selected) = tab.selected() {
                completion_strings.push(format!(" {}", selected.path.display()));
            }
        }
        completion_strings
    }
}

impl LineDisplay for InputSimple {
    fn line_display(&self, tab: &Tab) -> Vec<String> {
        match self {
            Self::Password(_, PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP)) => {
                vec![PasswordKind::CRYPTSETUP.to_string(), tab.input.password()]
            }
            Self::Password(_, _) => {
                vec![PasswordKind::SUDO.to_string(), tab.input.password()]
            }
            _ => {
                vec![Edit::InputSimple(*self).to_string(), tab.input.string()]
            }
        }
    }
}
