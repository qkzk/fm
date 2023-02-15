use std::io::prelude::*;
use std::io::Write;

use crate::fm_error::FmResult;
use crate::impl_selectable_content;
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
use lzma::LzmaWriter;
use tar::Builder;
use zip::write::FileOptions;

/// Different kind of compression methods
#[derive(Debug)]
pub enum CompressionMethod {
    ZIP,
    DEFLATE,
    GZ,
    ZLIB,
    LZMA,
}

/// Holds a vector of CompressionMethod and a few methods to compress some files.
#[derive(Debug)]
pub struct Compresser {
    content: Vec<CompressionMethod>,
    pub index: usize,
}

impl Compresser {
    /// Creates a new compresser.
    pub fn new() -> Self {
        Self {
            content: vec![
                CompressionMethod::ZIP,
                CompressionMethod::LZMA,
                CompressionMethod::GZ,
                CompressionMethod::ZLIB,
                CompressionMethod::DEFLATE,
            ],
            index: 0,
        }
    }

    /// Archive the files with tar and compress them with the selected method.
    /// The compression method is chosen by the user.
    pub fn compress(&self, files: Vec<std::path::PathBuf>) -> FmResult<()> {
        let Some(selected) = self.selected() else { return Ok(()) };
        match selected {
            CompressionMethod::DEFLATE => Self::compress_deflate("archive.tar.gz", files),
            CompressionMethod::GZ => Self::compress_gzip("archive.tar.gz", files),
            CompressionMethod::ZLIB => Self::compress_zlib("archive.tar.xz", files),
            CompressionMethod::ZIP => Self::compress_zip("archive.zip", files),
            CompressionMethod::LZMA => Self::compress_lzma("archive.tar.xz", files),
        }
    }

    fn compress_gzip(archive_name: &str, files: Vec<std::path::PathBuf>) -> FmResult<()> {
        let compressed_file = std::fs::File::create(archive_name)?;
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

    fn compress_deflate(archive_name: &str, files: Vec<std::path::PathBuf>) -> FmResult<()> {
        let compressed_file = std::fs::File::create(archive_name)?;
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

        // Finish deflate file
        encoder.finish()?;

        Ok(())
    }

    fn compress_zlib(archive_name: &str, files: Vec<std::path::PathBuf>) -> FmResult<()> {
        let compressed_file = std::fs::File::create(archive_name)?;
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

        // Finish zlib file
        encoder.finish()?;

        Ok(())
    }

    fn compress_zip(archive_name: &str, files: Vec<std::path::PathBuf>) -> FmResult<()> {
        let archive = std::fs::File::create(&archive_name).unwrap();
        let mut zip = zip::ZipWriter::new(archive);
        for file in files.iter() {
            zip.start_file(
                file.to_str().unwrap(),
                FileOptions::default().compression_method(zip::CompressionMethod::Bzip2),
            )?;
            let mut buffer = Vec::new();
            let mut content = std::fs::File::open(file)?;
            content.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        }

        // Finish zip file
        zip.finish()?;
        Ok(())
    }

    fn compress_lzma(archive_name: &str, files: Vec<std::path::PathBuf>) -> FmResult<()> {
        let compressed_file = std::fs::File::create(archive_name)?;
        let mut encoder = LzmaWriter::new_compressor(compressed_file, 6)?;
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

        // Finish lzma file
        encoder.finish()?;

        Ok(())
    }
}

impl_selectable_content!(CompressionMethod, Compresser);
