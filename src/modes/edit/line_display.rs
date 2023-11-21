use anyhow::{Context, Result};

use crate::app::Tab;
use crate::modes::{
    EditMode, InputCompleted, InputSimple, MarkAction, Navigate, NeedConfirmation, PasswordKind,
    PasswordUsage,
};

pub trait LineDisplay {
    fn line_display(&self, tab: &Tab) -> Result<Vec<String>>;
}

impl LineDisplay for EditMode {
    fn line_display(&self, tab: &Tab) -> Result<Vec<String>> {
        match self {
            EditMode::Navigate(mode) => mode.line_display(tab),
            EditMode::InputSimple(mode) => mode.line_display(tab),
            EditMode::InputCompleted(mode) => mode.line_display(tab),
            EditMode::NeedConfirmation(mode) => mode.line_display(tab),
            EditMode::Nothing => Ok(vec![]),
        }
    }
}

impl LineDisplay for NeedConfirmation {
    fn line_display(&self, _tab: &Tab) -> Result<Vec<String>> {
        Ok(vec![format!("{self}"), " (y/n)".to_owned()])
    }
}

impl LineDisplay for Navigate {
    fn line_display(&self, _tab: &Tab) -> Result<Vec<String>> {
        let content = match self {
            Navigate::Marks(MarkAction::Jump) => {
                vec!["Jump to...".to_owned()]
            }
            Navigate::Marks(MarkAction::New) => {
                vec!["Save mark...".to_owned()]
            }
            _ => {
                vec![EditMode::Navigate(*self).to_string()]
            }
        };
        Ok(content)
    }
}

impl LineDisplay for InputCompleted {
    fn line_display(&self, tab: &Tab) -> Result<Vec<String>> {
        let mut completion_strings = vec![tab.edit_mode.to_string(), tab.input.string()];
        if let Some(completion) = tab.completion.complete_input_string(&tab.input.string()) {
            completion_strings.push(completion.to_owned())
        }
        if let InputCompleted::Exec = *self {
            let selected_path = &tab.selected().context("can't parse path")?.path;
            let selected_path = format!(" {}", selected_path.display());

            completion_strings.push(selected_path);
        }
        Ok(completion_strings)
    }
}

impl LineDisplay for InputSimple {
    fn line_display(&self, tab: &Tab) -> Result<Vec<String>> {
        let content = match self {
            InputSimple::Password(_, PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP)) => {
                vec![PasswordKind::CRYPTSETUP.to_string(), tab.input.password()]
            }
            InputSimple::Password(_, _) => {
                vec![PasswordKind::SUDO.to_string(), tab.input.password()]
            }
            _ => {
                vec![EditMode::InputSimple(*self).to_string(), tab.input.string()]
            }
        };
        Ok(content)
    }
}
