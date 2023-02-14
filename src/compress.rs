use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
use std::fs::File;
use tar::Builder;

// use crate::fileinfo::FileInfo;
use crate::fm_error::FmResult;

pub fn compressed_gzip(archive_name: String, files: Vec<std::path::PathBuf>) -> FmResult<()> {
    let compressed_file = File::create(archive_name)?;
    let mut encoder = GzEncoder::new(compressed_file, Compression::default());

    {
        // Create tar archive and compress files
        let mut archive = Builder::new(&mut encoder);
        for file in files.iter() {
            if file.is_dir() {
                archive.append_dir_all(&file, &file)?;
            } else {
                archive.append_path(&file)?;
            }
        }
    }

    // Finish Gzip file
    encoder.finish()?;

    Ok(())
}

pub fn compressed_deflate(archive_name: String, files: Vec<std::path::PathBuf>) -> FmResult<()> {
    let compressed_file = File::create(archive_name)?;
    let mut encoder = DeflateEncoder::new(compressed_file, Compression::default());

    {
        // Create tar archive and compress files
        let mut archive = Builder::new(&mut encoder);
        for file in files.iter() {
            if file.is_dir() {
                archive.append_dir_all(&file, &file)?;
            } else {
                archive.append_path(&file)?;
            }
        }
    }

    // Finish Gzip file
    encoder.finish()?;

    Ok(())
}

pub fn compressed_zlib(archive_name: String, files: Vec<std::path::PathBuf>) -> FmResult<()> {
    let compressed_file = File::create(archive_name)?;
    let mut encoder = ZlibEncoder::new(compressed_file, Compression::default());

    {
        // Create tar archive and compress files
        let mut archive = Builder::new(&mut encoder);
        for file in files.iter() {
            if file.is_dir() {
                archive.append_dir_all(&file, &file)?;
            } else {
                archive.append_path(&file)?;
            }
        }
    }

    // Finish Gzip file
    encoder.finish()?;

    Ok(())
}
