use anyhow::Result;

use crate::app::Status;
use crate::common::path_to_string;

/// Expanded tokens from a configured command.
/// %s is converted into a `Selected`
/// %f is converted into a `Flagged`
/// %e is converted into a `Extension`
/// %n is converted into a `Filename`
/// Everything else is left intact and wrapped into an `Arg(string)`.
#[derive(Debug, Clone)]
pub enum Token {
    Arg(String),
    Extension,
    Filename,
    Flagged,
    Path,
    Selected,
}

impl Token {
    fn from(arg: &str) -> Self {
        match arg {
            "%s" => Self::Selected,
            "%e" => Self::Extension,
            "%n" => Self::Filename,
            "%f" => Self::Flagged,
            "%d" => Self::Path,
            _ => Self::Arg(arg.to_owned()),
        }
    }
}

/// Parse a command defined in the config file into a list of tokens
/// Those tokens are converted back into a list of arguments to be run
#[derive(Debug, Clone)]
pub struct ShellCommandParser {
    parsed: Vec<Token>,
}

impl ShellCommandParser {
    /// Parse a command into a list of tokens
    #[must_use]
    pub fn new(command: &str) -> Self {
        Self {
            parsed: Self::parse(command),
        }
    }

    fn parse(command: &str) -> Vec<Token> {
        command.split(' ').map(Token::from).collect()
    }

    /// Compute the command back into an arg list to be executed.
    ///
    /// # Errors
    ///
    /// May fail if :
    /// - The current directory name can't be decoded to utf-8
    /// - The selected filename can't be decoded to utf-8
    /// - The directory is empty
    /// - The file extention can't be decoded to utf-8
    pub fn compute(&self, status: &Status) -> Result<Vec<String>> {
        let mut computed = vec![];
        for token in &self.parsed {
            match token {
                Token::Arg(string) => computed.push(string.clone()),
                Token::Selected => {
                    computed.push(Self::selected(status)?);
                }
                Token::Path => {
                    computed.push(Self::path(status));
                }
                Token::Filename => {
                    computed.push(Self::filename(status)?);
                }
                Token::Extension => {
                    computed.push(Self::extension(status)?);
                }
                Token::Flagged => computed.extend_from_slice(&Self::flagged(status)),
            }
        }
        Ok(computed)
    }

    fn selected(status: &Status) -> Result<String> {
        status.current_tab().current_file_string()
    }

    fn path(status: &Status) -> String {
        status.current_tab().directory_str()
    }

    fn filename(status: &Status) -> Result<String> {
        Ok(status.current_tab().current_file()?.filename.to_string())
    }

    fn extension(status: &Status) -> Result<String> {
        Ok(status.current_tab().current_file()?.extension.to_string())
    }

    fn flagged(status: &Status) -> Vec<String> {
        status
            .menu
            .flagged
            .content
            .iter()
            .map(path_to_string)
            .collect()
    }
}
