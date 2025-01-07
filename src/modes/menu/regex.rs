use std::path::Path;

use anyhow::Result;
use regex::Regex;

use crate::common::filename_from_path;
use crate::modes::Flagged;

/// Flag every file matching a typed regex in current directory.
///
/// # Errors
///
/// It may fail if the `input_string` can't be parsed as a regex expression.
/// It may also fail if a file in the directory has a filename which can't be decoded as utf-8.
pub fn regex_flagger(input_string: &str, paths: &[&Path], flagged: &mut Flagged) -> Result<()> {
    let Ok(regex) = CaseDependantRegex::new(input_string) else {
        return Ok(());
    };
    flagged.clear();
    for path in paths {
        if regex.is_match(filename_from_path(path)?) {
            flagged.push(path.to_path_buf());
        }
    }

    Ok(())
}

/// Case dependant regular expression.
///
/// It holds an input string (the original regular expression) and a regular expression.
/// If the input string contains an uppercase character, we use it as is.
/// If not, we make the regular expression case insensitive by adding `(?i)` in front of it.
///
/// If the input is "Car" it will match against "Car", "Cargo" but not "car".
/// If the input is "car" it will match against "Car", "Cargo" and "car".
///
/// This is inspired by ranger which take it from vim/neovim.
#[derive(Clone)]
pub struct CaseDependantRegex {
    input_string: String,
    regex: Regex,
}

impl CaseDependantRegex {
    /// Creates a new case dependant regular expression.
    ///
    /// # Errors
    ///
    /// It may fail if the input_string can't be parsed as a regular expression.
    pub fn new(input_string: &str) -> Result<Self> {
        Ok(Self {
            input_string: input_string.to_string(),
            regex: Self::complete_regex(input_string)?,
        })
    }

    /// True if the input string is empty.
    pub fn is_empty(&self) -> bool {
        self.input_string.is_empty()
    }

    /// True if the regular expression matches the haystack.
    pub fn is_match(&self, haystack: &str) -> bool {
        self.regex.is_match(haystack)
    }

    fn complete_regex(input_string: &str) -> Result<Regex> {
        let re = if Self::has_uppercase(input_string) {
            input_string
        } else {
            &format!("(?i){input_string}")
        };
        Ok(Regex::new(re)?)
    }

    fn has_uppercase(input_string: &str) -> bool {
        input_string.chars().any(|c| c.is_uppercase())
    }
}

impl std::fmt::Display for CaseDependantRegex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.input_string)
    }
}
