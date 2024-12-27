use std::borrow::Cow;
use std::io::{prelude::*, Write};
use std::path::{Path, PathBuf};

use anyhow::bail;
use anyhow::Result;
use flate2::write::{DeflateEncoder, GzEncoder, ZlibEncoder};
use flate2::Compression;
use lzma::LzmaWriter;
use sevenz_rust::compress_to_path;
use zip::write::SimpleFileOptions;

use crate::io::{CowStr, DrawMenu};
use crate::{impl_content, impl_selectable, log_info, log_line};

/// Different kind of compression methods
#[derive(Debug)]
pub enum CompressionMethod {
    Zip,
    Defl,
    Gz,
    Zlib,
    Lzma,
    Sevenz,
}

impl CompressionMethod {
    fn to_str(&self) -> &'static str {
        match self {
            Self::Zip => "ZIP:     archive.zip",
            Self::Defl => "DEFLATE: archive.tar.gz",
            Self::Lzma => "LZMA:    archive.tar.xz",
            Self::Gz => "GZ:      archive.tar.gz",
            Self::Zlib => "ZLIB:    archive.tar.xz",
            Self::Sevenz => "7Z:      archive.7z",
        }
    }
}

/// Holds a vector of CompressionMethod and a few methods to compress some files.
#[derive(Debug, Default)]
pub struct Compresser {
    content: Vec<CompressionMethod>,
    pub index: usize,
}

impl Compresser {
    pub fn setup(&mut self) {
        self.content = vec![
            CompressionMethod::Zip,
            CompressionMethod::Lzma,
            CompressionMethod::Zlib,
            CompressionMethod::Gz,
            CompressionMethod::Defl,
            CompressionMethod::Sevenz,
        ];
    }
    /// Archive the files with tar and compress them with the selected method.
    /// The compression method is chosen by the user.
    /// Archive is created `here` which should be the path of the selected tab.
    pub fn compress(&self, files: Vec<PathBuf>, here: &Path) -> Result<()> {
        let Some(selected) = self.selected() else {
            return Ok(());
        };
        match selected {
            #[rustfmt::skip]
            CompressionMethod::Zip  => Self::zip (Self::archive(here, "archive.zip")?, files)?,
            CompressionMethod::Lzma => Self::lzma(Self::archive(here, "archive.tar.xz")?, files)?,
            CompressionMethod::Zlib => Self::zlib(Self::archive(here, "archive.tar.xz")?, files)?,
            CompressionMethod::Gz => Self::gzip(Self::archive(here, "archive.tar.gz")?, files)?,
            CompressionMethod::Defl => Self::defl(Self::archive(here, "archive.tar.gz")?, files)?,
            CompressionMethod::Sevenz => Self::sevenz(here, "archive.7z", files)?,
        }
        log_line!("Compressed with {selected}", selected = selected.to_str());
        Ok(())
    }

    fn make_tar<W>(files: Vec<PathBuf>, mut archive: tar::Builder<W>) -> Result<()>
    where
        W: Write,
    {
        for path in files.iter() {
            if path.starts_with("..") {
                continue;
            }
            if path.is_dir() {
                archive.append_dir_all(path, path)?;
            } else {
                archive.append_path(path)?;
            }
        }
        Ok(())
    }

    fn archive(here: &Path, archive_name: &str) -> Result<std::fs::File> {
        let mut full_path = here.to_path_buf();
        full_path.push(archive_name);
        Ok(std::fs::File::create(full_path)?)
    }

    fn gzip(archive: std::fs::File, files: Vec<PathBuf>) -> Result<()> {
        let mut encoder = GzEncoder::new(archive, Compression::default());

        // Create tar archive and compress files
        Self::make_tar(files, tar::Builder::new(&mut encoder))?;

        // Finish Gzip file
        encoder.finish()?;

        Ok(())
    }

    fn defl(archive: std::fs::File, files: Vec<PathBuf>) -> Result<()> {
        let mut encoder = DeflateEncoder::new(archive, Compression::default());

        // Create tar archive and compress files
        Self::make_tar(files, tar::Builder::new(&mut encoder))?;

        // Finish deflate file
        encoder.finish()?;

        Ok(())
    }

    fn zlib(archive: std::fs::File, files: Vec<PathBuf>) -> Result<()> {
        let mut encoder = ZlibEncoder::new(archive, Compression::default());

        // Create tar archive and compress files
        Self::make_tar(files, tar::Builder::new(&mut encoder))?;

        // Finish zlib file
        encoder.finish()?;

        Ok(())
    }

    fn lzma(archive: std::fs::File, files: Vec<PathBuf>) -> Result<()> {
        let mut encoder = LzmaWriter::new_compressor(archive, 6)?;

        // Create tar archive and compress files
        Self::make_tar(files, tar::Builder::new(&mut encoder))?;

        // Finish lzma file
        encoder.finish()?;

        Ok(())
    }

    fn zip(archive: std::fs::File, files: Vec<PathBuf>) -> Result<()> {
        let mut zip = zip::ZipWriter::new(archive);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755)
            .compression_method(zip::CompressionMethod::Bzip2);
        for file in files.iter() {
            zip.start_file(file.to_string_lossy().as_ref(), options)?;
            let mut buffer = Vec::new();
            let mut content = std::fs::File::open(file)?;
            content.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        }

        // Finish zip file
        zip.finish()?;
        Ok(())
    }

    fn sevenz(dest: &Path, filename: &str, files: Vec<PathBuf>) -> Result<()> {
        if files.len() != 1 {
            let error_msg = "Can only compress a single file with 7z compression method.";
            log_line!("{error_msg}");
            log_info!("{error_msg}");
            bail!("{error_msg}")
        }
        compress_to_path(&files[0], dest.join(filename))?;
        Ok(())
    }
}

impl_selectable!(Compresser);
impl_content!(CompressionMethod, Compresser);

impl CowStr for CompressionMethod {
    fn cow_str(&self) -> Cow<str> {
        self.to_str().into()
    }
}

impl DrawMenu<CompressionMethod> for Compresser {}
