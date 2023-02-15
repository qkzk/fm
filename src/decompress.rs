use crate::fm_error::{FmError, FmResult};
use flate2::read::{GzDecoder, ZlibDecoder};
use std::fs::File;
use std::path::Path;
use tar::Archive;

/// Decompress a zipped compressed file into its parent directory.
/// It may fail an return a `FmError` if the file has no parent,
/// which should be impossible.
pub fn decompress_zip(source: &Path) -> FmResult<()> {
    let file = File::open(source)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let parent = source
        .parent()
        .ok_or_else(|| FmError::custom("decompress", "source should have a parent"))?;
    zip.extract(parent)?;

    Ok(())
}

/// Decompress a gz compressed file into its parent directory.
/// It may fail an return a `FmError` if the file has no parent,
/// which should be impossible.
pub fn decompress_gz(source: &Path) -> FmResult<()> {
    let tar_gz = File::open(source)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    let parent = source
        .parent()
        .ok_or_else(|| FmError::custom("decompress", "source should have a parent"))?;
    archive.unpack(parent)?;

    Ok(())
}

/// Decompress a zlib compressed file into its parent directory.
/// It may fail an return a `FmError` if the file has no parent,
/// which should be impossible.
pub fn decompress_xz(source: &Path) -> FmResult<()> {
    let tar_xz = File::open(source)?;
    let tar = ZlibDecoder::new(tar_xz);
    let mut archive = Archive::new(tar);
    let parent = source
        .parent()
        .ok_or_else(|| FmError::custom("decompress", "source should have a parent"))?;
    archive.unpack(parent)?;

    Ok(())
}

pub fn list_files_zip<P>(source: P) -> FmResult<Vec<String>>
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
