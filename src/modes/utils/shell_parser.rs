use anyhow::{bail, Result};

use crate::app::Status;
use crate::common::{get_clipboard, path_to_string};
use crate::{log_info, log_line};

pub const SAME_WINDOW_TOKEN: &str = "%t";

/// Analyse, parse and builds arguments from a shell command.
/// Normal commands are executed with `sh -c "command"` which allow redirection, pipes etc.
/// Sudo commands are executed with `sudo` then `sh -c "rest of the command"`.
/// The password will be asked, injected into stdin and dropped somewhere else.
/// The command isn't executed here, we just build a list of arguments to be passed to an executer.
///
/// Some expansion are allowed to interact with the content of fm.
/// Expanded tokens from a configured command.
/// %s is converted into a `Selected`
/// %f is converted into a `Flagged`
/// %e is converted into a `Extension`
/// %n is converted into a `Filename`
/// %t is converted into a `$TERM` + custom flag.
/// %c is converted into a `Clipboard content`.
/// Everything else is left intact and wrapped into an `Arg(string)`.
///
/// # Errors
///
/// It can fail if the command can't be analysed or the expansion aren't valid (see above).
pub fn shell_command_parser(command: &str, status: &Status) -> Result<Vec<String>> {
    let Ok(tokens) = Lexer::new(command).lexer() else {
        return shell_command_parser_error("Syntax error in the command", command);
    };
    let Ok(args) = Parser::new(tokens).parse(status) else {
        return shell_command_parser_error("Couldn't parse the command", command);
    };
    build_args(args)
}

fn shell_command_parser_error(message: &str, command: &str) -> Result<Vec<String>> {
    log_info!("{message} {command}");
    log_line!("{message} {command}");
    bail!("{message} {command}");
}

#[derive(Debug)]
enum Token {
    Identifier(String),
    StringLiteral((char, String)),
    FmExpansion(FmExpansion),
}

#[derive(Debug)]
enum FmExpansion {
    Selected,
    SelectedFilename,
    SelectedPath,
    Extension,
    Flagged,
    Term,
    Clipboard,
    Invalid,
}

impl FmExpansion {
    fn from(c: char) -> Self {
        match c {
            's' => Self::Selected,
            'n' => Self::SelectedFilename,
            'd' => Self::SelectedPath,
            'e' => Self::Extension,
            'f' => Self::Flagged,
            't' => Self::Term,
            'c' => Self::Clipboard,
            _ => Self::Invalid,
        }
    }

    fn parse(&self, status: &Status) -> Result<Vec<String>> {
        match self {
            Self::Invalid => bail!("Invalid Fm Expansion"),
            Self::Term => Self::term(status),
            Self::Selected => Self::selected(status),
            Self::Flagged => Self::flagged(status),
            Self::SelectedPath => Self::path(status),
            Self::SelectedFilename => Self::filename(status),
            Self::Clipboard => Self::clipboard(),
            Self::Extension => Self::extension(status),
        }
    }

    fn selected(status: &Status) -> Result<Vec<String>> {
        Ok(vec![status.current_tab().current_file_string()?])
    }

    fn path(status: &Status) -> Result<Vec<String>> {
        Ok(vec![status.current_tab().directory_str()])
    }

    fn filename(status: &Status) -> Result<Vec<String>> {
        Ok(vec![status
            .current_tab()
            .current_file()?
            .filename
            .to_string()])
    }

    fn extension(status: &Status) -> Result<Vec<String>> {
        Ok(vec![status
            .current_tab()
            .current_file()?
            .extension
            .to_string()])
    }

    fn flagged(status: &Status) -> Result<Vec<String>> {
        Ok(status
            .menu
            .flagged
            .content
            .iter()
            .map(path_to_string)
            .collect())
    }

    fn term(_status: &Status) -> Result<Vec<String>> {
        Ok(vec![SAME_WINDOW_TOKEN.to_owned()])
    }

    fn clipboard() -> Result<Vec<String>> {
        let Some(clipboard) = get_clipboard() else {
            bail!("Couldn't read the clipboard");
        };
        Ok(clipboard.split_whitespace().map(|s| s.to_owned()).collect())
    }
}

enum State {
    Start,
    Arg,
    StringLiteral(char),
    FmExpansion,
}

struct Lexer {
    command: String,
}

impl Lexer {
    fn new(command: &str) -> Self {
        Self {
            command: command.trim().to_owned(),
        }
    }

    fn lexer(&self) -> Result<Vec<Token>> {
        let mut tokens = vec![];
        let mut state = State::Start;
        let mut current = String::new();

        for c in self.command.chars() {
            match &state {
                State::Start => {
                    if c == '"' || c == '\'' {
                        state = State::StringLiteral(c);
                    } else if c == '%' {
                        state = State::FmExpansion;
                    } else {
                        state = State::Arg;
                        current.push(c);
                    }
                }
                State::Arg => {
                    if c == '%' {
                        tokens.push(Token::Identifier(current.clone()));
                        current.clear();
                        state = State::FmExpansion;
                    } else if c == '"' || c == '\'' {
                        tokens.push(Token::Identifier(current.clone()));
                        current.clear();
                        state = State::StringLiteral(c);
                    } else {
                        current.push(c);
                    }
                }
                State::StringLiteral(quote_type) => {
                    if c == *quote_type {
                        tokens.push(Token::StringLiteral((c, current.clone())));
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
                        if let FmExpansion::Term = expansion {
                            if !tokens.is_empty() {
                                bail!("Term expansion can only be the first argument")
                            }
                        }
                        tokens.push(Token::FmExpansion(expansion));
                        current.clear();
                        state = State::Start;
                    } else {
                        bail!("Invalid FmExpansion %{c}.")
                    }
                }
            }
        }

        match &state {
            State::Arg => tokens.push(Token::Identifier(current)),
            State::StringLiteral(quote) => tokens.push(Token::StringLiteral((*quote,current))),
            State::FmExpansion => bail!("Invalid syntax for {command}. Matching an FmExpansion with {current} which is impossible.", command=self.command),
            State::Start => (),
        }

        Ok(tokens)
    }
}

struct Parser {
    tokens: Vec<Token>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens }
    }

    fn parse(&self, status: &Status) -> Result<Vec<String>> {
        if self.tokens.is_empty() {
            bail!("Empty tokens")
        }
        let mut args: Vec<String> = vec![];
        for token in self.tokens.iter() {
            match token {
                Token::Identifier(identifier) => args.push(identifier.to_owned()),
                Token::FmExpansion(fm_expansion) => {
                    let Ok(mut expansion) = fm_expansion.parse(status) else {
                        log_line!("Invalid expansion {fm_expansion:?}");
                        log_info!("Invalid expansion {fm_expansion:?}");
                        bail!("Invalid expansion {fm_expansion:?}")
                    };
                    args.append(&mut expansion)
                }
                Token::StringLiteral((quote, string)) => {
                    args.push(format!("{quote}{string}{quote}"))
                }
            };
        }
        Ok(args)
    }
}

fn build_args(args: Vec<String>) -> Result<Vec<String>> {
    log_info!("build_args {args:?}");
    if args.is_empty() {
        bail!("Empty command");
    }
    if args[0].starts_with("sudo") {
        Ok(build_sudo_args(args))
    } else if args[0].starts_with(SAME_WINDOW_TOKEN) {
        Ok(args)
    } else {
        Ok(build_normal_args(args))
    }
}

fn build_sudo_args(args: Vec<String>) -> Vec<String> {
    let rebuild = args.join("");
    rebuild.split_whitespace().map(|s| s.to_owned()).collect()
}

fn build_normal_args(args: Vec<String>) -> Vec<String> {
    vec!["sh".to_owned(), "-c".to_owned(), args.join("")]
}
// fn test_shell_parser(status: &Status) {
//     let commands = vec![
//         r#"echo "Hello World" | grep "World""#, // Commande simple avec pipe et chaîne avec espaces
//         r#"ls -l /home/user && echo "Done""#, // Commande avec opérateur logique `&&` et chaîne avec espaces
//         r#"cat file.txt > output.txt"#,       // Redirection de sortie vers un fichier
//         r#"grep 'pattern' < input.txt | sort >> output.txt"#, // Redirections d'entrée et de sortie avec pipe
//         r#"echo "Unfinished quote"#,                          // Cas avec guillemet non fermé
//         r#"echo "Special chars: $HOME, * and ?""#, // Commande avec variables et jokers dans une chaîne
//         r#"rm -rf /some/directory || echo "Failed to delete""#, // Commande avec opérateur logique `||`
//         r#"find . -name "*.txt" | xargs grep "Hello""#, // Recherche de fichiers avec pipe et argument contenant `*`
//         r#"echo "Spaces   between   words""#,           // Chaîne avec plusieurs espaces
//         r#"echo Hello\ World"#, // Utilisation de `\` pour échapper un espace
//         r#"ls %s"#,
//         r#"bat %s --color=never | rg "main" --line-numbers"#,
//     ];
//
//     for command in &commands {
//         let tokens = Lexer::new(command).lexer();
//         // crate::log_info!("{command}\n--lexer-->\n{:?}\n", tokens);
//         if let Ok(tokens) = tokens {
//             let p = Parser::new(tokens).parse(status);
//             let c = build_command(p);
//             crate::log_info!("command: {c:?}\n");
//         }
//     }
// }
