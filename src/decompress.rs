use anyhow::{Context, Result};
use flate2::read::{GzDecoder, ZlibDecoder};
use std::fs::File;
use std::path::Path;
use tar::Archive;

use crate::constant_strings_paths::TAR;
use crate::utils::path_to_string;

/// Decompress a zipped compressed file into its parent directory.
pub fn decompress_zip(source: &Path) -> Result<()> {
    let file = File::open(source)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let parent = source
        .parent()
        .context("decompress: source should have a parent")?;
    zip.extract(parent)?;

    Ok(())
}

/// Decompress a gz compressed file into its parent directory.
pub fn decompress_gz(source: &Path) -> Result<()> {
    let tar_gz = File::open(source)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    let parent = source
        .parent()
        .context("decompress: source should have a parent")?;
    archive.unpack(parent)?;

    Ok(())
}

/// Decompress a zlib compressed file into its parent directory.
pub fn decompress_xz(source: &Path) -> Result<()> {
    let tar_xz = File::open(source)?;
    let tar = ZlibDecoder::new(tar_xz);
    let mut archive = Archive::new(tar);
    let parent = source
        .parent()
        .context("decompress: source should have a parent")?;
    archive.unpack(parent)?;

    Ok(())
}

/// List files contained in a ZIP file.
/// Will return an error if the ZIP file is corrupted.
pub fn list_files_zip<P>(source: P) -> Result<Vec<String>>
where
    P: AsRef<Path>,
{
    let file = File::open(source)?;
    let zip = zip::ZipArchive::new(file)?;
    Ok(zip.file_names().map(|f| f.to_owned()).collect())
}

/// List files contained in a tar.something file.
/// Will return an error if `tar tvf source` can't list the content.
pub fn list_files_tar<P>(source: P) -> Result<Vec<String>>
where
    P: AsRef<Path>,
{
    if let Ok(output) = std::process::Command::new(TAR)
        .arg("tvf")
        .arg(path_to_string(&source))
        .output()
    {
        let output = String::from_utf8(output.stdout).unwrap_or_default();
        let content = output.lines().map(|l| l.to_owned()).collect();
        return Ok(content);
    }
    Err(anyhow::anyhow!("Tar couldn't read the file content"))
}
