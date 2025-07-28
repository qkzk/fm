use std::borrow::Cow;

use anyhow::Result;

/// Quote a string for shell insertion.
/// It is required to create commands like "zsh -c nvim <path>" where <path> may contain any kind of byte.
/// We only have problems with `'`, `"` and `\'.
/// Since it's the only usage, we don't really care about allocation.
pub trait Quote<S> {
    /// Shell quote a filepath to use it in _shell_ commands.
    /// It's the responsability of the caller to ensure this trait is only used for
    /// commands **executed by the shell itself**.
    /// Using it for normal commands isn't necessary and will create errors.
    ///
    /// # Errors
    ///
    /// It may fail if the string can't be read as an utf-8 string.
    /// For the most common case (&str, Cow<str>, String) it's impossible.
    fn quote(&self) -> Result<S>;
}

impl Quote<String> for String {
    fn quote(&self) -> Result<String> {
        self.as_str().quote()
    }
}

impl Quote<String> for &str {
    fn quote(&self) -> Result<String> {
        try_quote(self)
    }
}

impl Quote<String> for Cow<'_, str> {
    fn quote(&self) -> Result<String> {
        try_quote(self)
    }
}

fn must_quote(byte: u8) -> bool {
    matches!(byte, b' ' | b'\'' | b'"')
}

/// Quote a path to insert it into a shell command if need be.
/// Inspired by [yazi](https://github.com/sxyazi/yazi/blob/main/yazi-shared/src/shell/unix.rs)
fn try_quote(s: &str) -> Result<String> {
    if !s.bytes().any(must_quote) {
        Ok(s.into())
    } else {
        let len = s.len();
        let mut escaped = Vec::with_capacity(len + 2);
        escaped.push(b'\'');
        for byte in s.bytes() {
            match byte {
                b'\'' | b'"' => {
                    escaped.reserve(4);
                    escaped.push(b'\'');
                    escaped.push(b'\\');
                    escaped.push(byte);
                    escaped.push(b'\'');
                }
                _ => escaped.push(byte),
            }
        }
        escaped.push(b'\'');
        let s = String::from_utf8(escaped)?;
        crate::log_info!("try quote: #{s}#");
        Ok(s)
    }
}

/// Used to shell quote every filepath of a vector of string.
pub trait JoinQuote {
    fn join_quote(&self, sep: &str) -> String;
}

impl JoinQuote for &Vec<String> {
    /// Quote every _filepath_ in the vector and join them.
    fn join_quote(&self, sep: &str) -> String {
        self.iter()
            .filter_map(|fp| fp.quote().ok())
            .collect::<Vec<_>>()
            .join(sep)
    }
}
