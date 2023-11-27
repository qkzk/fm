use anyhow::{Context, Result};

use crate::app::Status;

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
            "%p" => Self::Path,
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
                    computed.push(Self::path(status)?);
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
        status
            .selected_non_mut()
            .path_content
            .selected_path_string()
            .context("Empty directory")
    }

    fn path(status: &Status) -> Result<String> {
        Ok(status
            .selected_non_mut()
            .path_content_str()
            .context("Couldn't read path")?
            .to_owned())
    }

    fn filename(status: &Status) -> Result<String> {
        Ok(status
            .selected_non_mut()
            .selected()
            .context("Empty directory")?
            .filename)
    }

    fn extension(status: &Status) -> Result<String> {
        Ok(status
            .selected_non_mut()
            .selected()
            .context("Empty directory")?
            .extension)
    }

    fn flagged(status: &Status) -> Vec<String> {
        status
            .menu
            .flagged
            .content
            .iter()
            .map(|path| path.to_str())
            .filter(std::option::Option::is_some)
            .map(|s| s.unwrap().to_owned())
            .collect()
    }
}
