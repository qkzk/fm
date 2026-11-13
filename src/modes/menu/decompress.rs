use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    common::{filename_from_path, is_in_path, path_to_string, GZ, SEVENZ, UNZIP},
    io::execute,
};
use crate::{
    common::{TAR, XZ},
    io::execute_without_output,
};
use crate::{log_info, log_line};

/// Decompress a zip compressed file into its parent directory.
///
/// # Errors
///
/// It may fail if the file can't be opened.
pub fn decompress_zip(source: &Path) -> Result<()> {
    log_info!("decompress_zip");
    if !is_in_path(UNZIP) {
        log_info!(
            "Can't decompress {source} without {UNZIP} executable",
            source = source.display()
        );
        log_line!(
            "Can't decompress {source} without {UNZIP} executable",
            source = source.display()
        );
        return Ok(());
    }
    let parent = source
        .parent()
        .context("decompress: source should have a parent")?;
    let args: &[&str] = &[
        &path_to_string(&source),
        "-d",
        &parent.display().to_string(),
    ];
    let _ = execute(UNZIP, args, true);
    Ok(())
}

/// Decompress a gz compressed file into its parent directory.
///
/// # Errors
///
/// It may fail if the file can't be opened.
pub fn decompress_gz(source: &Path) -> Result<()> {
    if filename_from_path(source)?.ends_with(".tar.gz") {
        decompress_tar(source, "zf")
    } else {
        decompress_single_gz(source)
    }
}

/// Decompress a xz compressed file into its parent directory.
///
/// # Errors
///
/// It may fail if the file can't be opened.
pub fn decompress_xz(source: &Path) -> Result<()> {
    if filename_from_path(source)?.ends_with(".tar.xz") {
        decompress_tar(source, "f")
    } else {
        decompress_single_xz(source)
    }
}

fn decompress_single_gz(source: &Path) -> Result<()> {
    if !is_in_path(GZ) {
        log_info!(
            "Can't decompress {source} without {GZ} executable",
            source = source.display()
        );
        log_line!(
            "Can't decompress {source} without {GZ} executable",
            source = source.display()
        );
        return Ok(());
    }
    let args = &["-d", "-k", &path_to_string(&source)];
    let _ = execute_without_output(GZ, args);
    Ok(())
}

fn decompress_single_xz(source: &Path) -> Result<()> {
    if !is_in_path(XZ) {
        log_info!(
            "Can't decompress {source} without {XZ} executable",
            source = source.display()
        );
        log_line!(
            "Can't decompress {source} without {XZ} executable",
            source = source.display()
        );
        return Ok(());
    }
    let args = &["-d", "-k", &path_to_string(&source)];
    let _ = execute_without_output(XZ, args);
    Ok(())
}

fn decompress_tar(source: &Path, flag: &str) -> Result<()> {
    if !is_in_path(TAR) {
        log_info!(
            "Can't decompress {source} without {TAR} executable",
            source = source.display()
        );
        log_line!(
            "Can't decompress {source} without {TAR} executable",
            source = source.display()
        );
        return Ok(());
    }
    let parent = source
        .parent()
        .context("decompress: source should have a parent")?;
    let args = &[
        &format!("-x{flag}"),
        &source.display().to_string(),
        "-C",
        &parent.display().to_string(),
    ];
    let _ = execute_without_output(TAR, args);
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
