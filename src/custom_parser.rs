use anyhow::{Context, Result};

use crate::status::Status;

/// Expanded tokens from a configured command.
/// %s is converted into a `Selected`
/// %f is converted into a `Flagged`
/// Everything else is left intact into an `Arg(string)`.
#[derive(Debug, Clone)]
pub enum Token {
    Arg(String),
    Flagged,
    Selected,
}

impl Token {
    fn from(arg: &str) -> Self {
        match arg {
            "%s" => Self::Selected,
            "%f" => Self::Flagged,
            _ => Self::Arg(arg.to_owned()),
        }
    }
}

/// Parse a command defined in the config file into a list of tokens
/// Those tokens are converted back into a list of arguments to be run
#[derive(Debug, Clone)]
pub struct CustomParser {
    parsed: Vec<Token>,
}

impl CustomParser {
    /// Parse a command into a list of tokens
    pub fn new(command: String) -> Self {
        Self {
            parsed: Self::parse(&command),
        }
    }

    fn parse(command: &str) -> Vec<Token> {
        command.split(' ').map(Token::from).collect()
    }

    /// Compute the command back into an arg list to be executed.
    pub fn compute(&self, status: &Status) -> Result<Vec<String>> {
        let mut computed = vec![];
        for token in self.parsed.iter() {
            match token {
                Token::Arg(string) => computed.push(string.to_owned()),
                Token::Selected => {
                    let path = status
                        .selected_non_mut()
                        .path_content
                        .selected_path_string()
                        .context("Empty directory")?;
                    computed.push(path);
                }
                Token::Flagged => {
                    for path in status.flagged.content.iter() {
                        computed.push(path.to_str().context("Couldn't parse the path")?.to_owned());
                    }
                }
            }
        }
        Ok(computed)
    }
}
