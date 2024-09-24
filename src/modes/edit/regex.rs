use std::path::Path;

use anyhow::Result;

use crate::common::filename_from_path;
use crate::modes::edit::Flagged;

/// Flag every file matching a typed regex in current directory.
///
/// # Errors
///
/// It may fail if the `input_string` can't be parsed as a regex expression.
/// It may also fail if a file in the directory has a filename which can't be decoded as utf-8.
pub fn regex_matcher(input_string: &str, paths: &[&Path], flagged: &mut Flagged) -> Result<()> {
    let Ok(re) = regex::Regex::new(input_string) else {
        return Ok(());
    };
    flagged.clear();
    for path in paths {
        if re.is_match(filename_from_path(path)?) {
            flagged.push(path.to_path_buf());
        }
    }

    Ok(())
}
