use std::fs::File;
use std::path::{Path, PathBuf};

use compress_tools::*;

use crate::fm_error::{FmError, FmResult};

/// Decompress a compressed file into its parent directory.
/// It may fail an return a `FmError` if the file has no parent,
/// which should be impossible.
/// It used `compress_tools` which is a wrapper around  `libarchive`.
pub fn decompress(source: PathBuf) -> FmResult<()> {
    let parent = source
        .parent()
        .ok_or_else(|| FmError::custom("decompress", "source should have a parent"))?;
    let file = File::open(&source)?;
    Ok(uncompress_archive(&file, parent, Ownership::Preserve)?)
}

/// Returns a list of compressed files within the archive.
/// it may fail if the file can't be opened or if libarchive
/// can't read it.
pub fn list_files<P>(source: P) -> FmResult<Vec<String>>
where
    P: AsRef<Path>,
{
    let file = File::open(source)?;
    Ok(list_archive_files(file)?)
}
