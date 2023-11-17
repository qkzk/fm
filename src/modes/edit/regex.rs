use std::path::Path;

use anyhow::Result;

use crate::{common::filename_from_path, modes::edit::Flagged};

pub fn regex_matcher(input_string: String, paths: &[&Path], flagged: &mut Flagged) -> Result<()> {
    flagged.clear();
    let re = regex::Regex::new(&input_string)?;
    for path in paths.iter() {
        if re.is_match(filename_from_path(path)?) {
            flagged.push(path.to_path_buf())
        }
    }

    Ok(())
}
