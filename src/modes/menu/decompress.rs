use std::{fs::File, path::Path};

use anyhow::{Context, Result};
use flate2::read::{GzDecoder, ZlibDecoder};
use tar::Archive;

use crate::common::{is_in_path, path_to_string, BSDTAR, SEVENZ};
use crate::io::{execute_and_output, execute_without_output};
use crate::{log_info, log_line};

/// Decompress a zipped compressed file into its parent directory.
///
/// # Errors
///
/// It may fail if the file can't be opened or if [`zip::ZipArchive::new`] can't
/// read the archive.
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
///
/// # Errors
///
/// It may fail if the file can't be opened.
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

/// Decompress a 7z compressed file into its parent directory.
///
/// # Errors
///
/// It may fail if the file can't be opened.
pub fn decompress_7z(source: &Path) -> Result<()> {
    if !is_in_path(SEVENZ) {
        log_info!(
            "Can't decompress {source} without {SEVENZ} executable",
            source = source.display()
        );
        log_line!(
            "Can't decompress {source} without {SEVENZ} executable",
            source = source.display()
        );
        return Ok(());
    }
    let parent = source
        .parent()
        .context("decompress: source should have a parent")?;
    let args = &[
        "x",
        &path_to_string(&source),
        &format!("-o{parent}", parent = parent.display()),
        "-y",
        "-bd",
    ];
    let _ = execute_without_output(SEVENZ, args);
    Ok(())
}

/// Decompress a zlib compressed file into its parent directory.
///
/// # Errors
///
/// It may fail if the file can't be opened.
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
///
/// # Errors
///
/// It may fail if the source file can't be open.
/// It may also fail if [`zip::ZipArchive::new`] read the archive.
pub fn list_files_zip<P>(source: P) -> Result<Vec<String>>
where
    P: AsRef<Path>,
{
    let file = File::open(source)?;
    let zip = zip::ZipArchive::new(file)?;
    Ok(zip
        .file_names()
        .map(std::borrow::ToOwned::to_owned)
        .collect())
}

/// List files contained in a tar.something file.
/// Will return an error if `tar tvf source` can't list the content.
///
/// # Errors
///
/// It may fail if the `tar tvf` command returns an error.
pub fn list_files_tar<P>(source: P) -> Result<Vec<String>>
where
    P: AsRef<Path>,
{
    if let Ok(output) = execute_and_output(
        BSDTAR,
        ["-v", "--list", "--file", path_to_string(&source).as_str()],
    ) {
        let output = String::from_utf8(output.stdout).unwrap_or_default();
        let content = output.lines().map(std::borrow::ToOwned::to_owned).collect();
        return Ok(content);
    }
    Err(anyhow::anyhow!("Tar couldn't read the file content"))
}
