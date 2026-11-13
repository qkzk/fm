use std::{
    fs::File,
    io::{copy, Read},
    path::Path,
};

use anyhow::{Context, Result};
use flate2::read::{DeflateDecoder, GzDecoder, MultiGzDecoder, ZlibDecoder};
use tar::Archive;
use xz2::read::XzDecoder;

use crate::io::{execute_and_output, execute_without_output};
use crate::{
    common::{filename_from_path, is_in_path, path_to_string, BSDTAR, SEVENZ, UNZIP},
    io::execute,
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
        &format!("{parent}", parent = parent.display()),
    ];
    let _ = execute(UNZIP, args, true);
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

type Reader<'a> = &'a [fn(File) -> Box<dyn Read>];

/// Decompress an xz or gz compressed file into its parent directory.
///
/// # Errors
///
/// It may fail if the file can't be opened.
pub fn decompress_xz_gz(source: &Path) -> Result<()> {
    let extension = source
        .extension()
        .context("Source should have an extension")?
        .to_string_lossy();
    let source_is_archive = filename_from_path(source)?.ends_with(&format!(".tar.{extension}"));
    let parent = source
        .parent()
        .context("decompress: source should have a parent")?;

    let strategies: Reader = &[
        |f| Box::new(XzDecoder::new(f)),
        |f| Box::new(GzDecoder::new(f)),
        |f| Box::new(ZlibDecoder::new(f)),
        |f| Box::new(DeflateDecoder::new(f)),
        |f| Box::new(MultiGzDecoder::new(f)),
        |f| Box::new(f),
    ];

    for make_reader in strategies {
        let file = File::open(source)?;
        let mut reader = make_reader(file);
        if source_is_archive {
            if try_unpack(reader, parent) {
                return Ok(());
            }
        } else {
            let mut dest = source.to_path_buf();
            dest.set_extension("");
            if let Ok(mut file) = File::create(dest) {
                if copy(&mut reader, &mut file).is_ok() {
                    return Ok(());
                }
            }
        }
    }

    log_info!("Couldn't unpack the archive {}", source.display());
    log_line!("Couldn't unpack the archive {}", source.display());

    Ok(())
}

fn try_unpack<R: Read>(reader: R, dst: &Path) -> bool {
    let mut archive = Archive::new(reader);
    archive.unpack(dst).is_ok()
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
