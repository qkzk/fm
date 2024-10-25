use anyhow::{bail, Result};

use crate::app::Status;
use crate::common::path_to_string;

// enum Ops {
//     DoubleQuote,
//     SimpleQuote,
//     SimpleRedirLeft,
//     DoubleRedirLeft,
//     TripleRedirLeft,
//     RedirRight,
//     SimpleAmpersand,
//     DoubleAmpersand,
//     DoublePipe,
//     SimplePipe,
//     Star,
//     QuestionMark,
//     LeftCurlyBrace,
//     RightCurlyBrace,
//     Dollar,
//     Hash,
//     // TODO translate
//     PointVirgule,
//     Tilde,
//     AntiSlash,
// }
//
#[derive(Debug)]
enum Tkn {
    Identifier(String),
    StringLiteral(String),
    Operator(String),
    FmExpansion(FmExpansion),
    Space,
}

#[derive(Debug)]
enum FmExpansion {
    Selected,
    SelectedFilename,
    SelectedPath,
    Flagged,
    Term,
    Invalid,
}

impl FmExpansion {
    fn from(c: char) -> Self {
        match c {
            's' => Self::Selected,
            'n' => Self::SelectedFilename,
            'd' => Self::SelectedPath,
            'f' => Self::Flagged,
            't' => Self::Term,
            _ => Self::Invalid,
        }
    }
}

enum State {
    Start,
    Identifier,
    StringLiteral(char),
    FmExpansion,
    Operator,
}

struct Lexer {
    command: String,
}

impl Lexer {
    fn new(command: &str) -> Self {
        Self {
            command: command.to_owned(),
        }
    }

    fn lexer(&self) -> Result<Vec<Tkn>> {
        let mut tokens = vec![];
        let mut state = State::Start;
        let mut current = String::new();

        for c in self.command.chars() {
            match &state {
                State::Start => {
                    if c.is_whitespace() {
                        tokens.push(Tkn::Space);
                    } else if c == '"' || c == '\'' {
                        state = State::StringLiteral(c);
                    } else if c == '%' {
                        state = State::FmExpansion;
                    } else if c.is_alphanumeric() || c == '_' {
                        state = State::Identifier;
                        current.push(c);
                    } else {
                        state = State::Operator;
                        current.push(c);
                    }
                }
                State::Identifier => {
                    if c.is_whitespace() {
                        tokens.push(Tkn::Identifier(current.clone()));
                        current.clear();
                        state = State::Start;
                    } else if c == '"' || c == '\'' {
                        tokens.push(Tkn::Identifier(current.clone()));
                        current.clear();
                        state = State::StringLiteral(c);
                    } else {
                        current.push(c);
                    }
                }
                State::StringLiteral(quote_type) => {
                    if c == *quote_type {
                        tokens.push(Tkn::StringLiteral(current.clone()));
                        current.clear();
                        state = State::Start;
                    } else {
                        current.push(c);
                    }
                }
                State::FmExpansion => {
                    if c.is_alphanumeric() {
                        let expansion = FmExpansion::from(c);
                        if let FmExpansion::Invalid = expansion {
                            bail!("Invalid FmExpansion %{c}")
                        }
                        tokens.push(Tkn::FmExpansion(expansion));
                        current.clear();
                        state = State::Start;
                    } else {
                        bail!("Invalid FmExpansion %{c}.")
                    }
                }
                State::Operator => {
                    if c.is_whitespace() || c.is_alphanumeric() {
                        tokens.push(Tkn::Operator(current.clone()));
                        current.clear();
                        if c.is_alphanumeric() {
                            current.push(c);
                            state = State::Identifier;
                        } else {
                            state = State::Start;
                        }
                    } else {
                        current.push(c);
                    }
                }
            }
        }

        match &state {
            State::Identifier => tokens.push(Tkn::Identifier(current)),
            State::StringLiteral(_) => tokens.push(Tkn::StringLiteral(current)),
            State::Operator => tokens.push(Tkn::Operator(current)),
            State::FmExpansion => bail!("Invalid syntax for {command}. Matching an FmExpansion with {current} which is impossible.", command=self.command),
            State::Start => (),
        }

        Ok(tokens)
    }
}

pub fn test_lexer() {
    let commands = vec![
        r#"echo "Hello World" | grep "World""#, // Commande simple avec pipe et chaîne avec espaces
        r#"ls -l /home/user && echo "Done""#, // Commande avec opérateur logique `&&` et chaîne avec espaces
        r#"cat file.txt > output.txt"#,       // Redirection de sortie vers un fichier
        r#"grep 'pattern' < input.txt | sort >> output.txt"#, // Redirections d'entrée et de sortie avec pipe
        r#"echo "Unfinished quote"#,                          // Cas avec guillemet non fermé
        r#"echo "Special chars: $HOME, * and ?""#, // Commande avec variables et jokers dans une chaîne
        r#"rm -rf /some/directory || echo "Failed to delete""#, // Commande avec opérateur logique `||`
        r#"find . -name "*.txt" | xargs grep "Hello""#, // Recherche de fichiers avec pipe et argument contenant `*`
        r#"echo "Spaces   between   words""#,           // Chaîne avec plusieurs espaces
        r#"echo Hello\ World"#, // Utilisation de `\` pour échapper un espace
        r#"ls %s"#,
        r#"bat %s --color=never | rg "main" --line-numbers"#,
    ];

    for command in commands {
        let tokens = Lexer::new(command).lexer();
        println!("{command}\n--lexer-->\n{:?}\n", tokens);
    }
}

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
    Term,
}

impl Token {
    fn from(arg: &str) -> Self {
        match arg {
            "%s" => Self::Selected,
            "%e" => Self::Extension,
            "%n" => Self::Filename,
            "%f" => Self::Flagged,
            "%d" => Self::Path,
            "%t" => Self::Term,
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

    /// Read the command and build the token from the syntax
    fn lexer(&mut self) {
        todo!()
    }

    /// Parse the command into exe & args to be executed
    fn parser(&mut self) {
        todo!()
    }

    /// execute the command itself, returns its output
    fn executer(&mut self) {
        todo!()
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
                Token::Term => computed.extend_from_slice(&Self::term(status)),
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

    fn term(status: &Status) -> [String; 2] {
        [
            status.internal_settings.opener.terminal.to_owned(),
            status.internal_settings.opener.terminal_flag.to_owned(),
        ]
    }
}
