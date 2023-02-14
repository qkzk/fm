use std::fs::File;
use std::io::prelude::*;
use std::io::Write;

use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
use tar::Builder;
use zip::write::FileOptions;

use crate::fm_error::FmResult;
use crate::impl_selectable_content;

#[derive(Debug)]
pub enum CompressionMethod {
    DEFLATE,
    GZ,
    ZLIB,
    ZIP,
}

#[derive(Debug)]
pub struct CompressionPicker {
    content: Vec<CompressionMethod>,
    pub index: usize,
}

impl CompressionPicker {
    pub fn new() -> Self {
        Self {
            content: vec![
                CompressionMethod::DEFLATE,
                CompressionMethod::GZ,
                CompressionMethod::ZLIB,
                CompressionMethod::ZIP,
            ],
            index: 0,
        }
    }
}

impl_selectable_content!(CompressionMethod, CompressionPicker);

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

pub fn compressed_zip(archive_name: String, files: Vec<std::path::PathBuf>) -> FmResult<()> {
    let archive = std::fs::File::create(&archive_name).unwrap();
    let mut zip = zip::ZipWriter::new(archive);
    for file in files.iter() {
        let mut buffer = Vec::new();
        zip.start_file(file.to_str().unwrap(), FileOptions::default())?;
        let mut content = File::open(file)?;
        content.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;
    }
    zip.finish()?;
    Ok(())
}
