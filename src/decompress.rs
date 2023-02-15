use std::fs::File;
use std::path::Path;

use crate::fm_error::{FmError, FmResult};

/// Decompress a compressed file into its parent directory.
/// It may fail an return a `FmError` if the file has no parent,
/// which should be impossible.
/// It used `compress_tools` which is a wrapper around  `libarchive`.
pub fn decompress(source: &Path) -> FmResult<()> {
    let file = File::open(source)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let parent = source
        .parent()
        .ok_or_else(|| FmError::custom("decompress", "source should have a parent"))?;
    zip.extract(parent)?;

    Ok(())
}

pub fn list_files<P>(source: P) -> FmResult<Vec<String>>
where
    P: AsRef<Path>,
{
    let file = File::open(source)?;
    let mut content = vec![];
    let mut zip = zip::ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        content.push(file.name().to_owned());
    }
    Ok(content)
}
