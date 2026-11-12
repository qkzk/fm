use std::borrow::Cow;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::common::{is_in_path, SEVENZ, TAR, ZIP};
use crate::io::execute_without_output;
use crate::{impl_content, impl_draw_menu_with_char, impl_selectable, log_info, log_line};

/// Different kind of compression methods
#[derive(Debug)]
pub enum CompressionMethod {
    Zip,
    Gz,
    Zlib,
    Sevenz,
}

impl CompressionMethod {
    fn to_str(&self) -> &'static str {
        match self {
            Self::Zip => "ZIP:     archive.zip",
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
            CompressionMethod::Zlib,
            CompressionMethod::Gz,
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
            CompressionMethod::Zip  => Self::zip(here, "archive.zip", files)?,
            CompressionMethod::Zlib => Self::xz(here, "archive.tar.xz", files)?,
            CompressionMethod::Gz => Self::gz(here, "archive.tar.gz", files)?,
            CompressionMethod::Sevenz => Self::sevenz(here, "archive.7z", files)?,
        }
        log_line!("Compressed with {selected}", selected = selected.to_str());
        Ok(())
    }

    fn xz(dest: &Path, filename: &str, files: Vec<PathBuf>) -> Result<()> {
        if !is_in_path(TAR) {
            log_info!("Can't compress with gzip without {TAR} executable");
            log_line!("Can't compress with gzip without {TAR} executable");
            return Ok(());
        }
        let dest = dest.join(filename);
        let dest = dest.to_str().context("")?;
        let mut args = vec!["-cJf", &dest];
        args.extend(files.iter().filter_map(|file| file.to_str()));
        let _ = execute_without_output(TAR, &args);
        Ok(())
    }

    fn zip(dest: &Path, filename: &str, files: Vec<PathBuf>) -> Result<()> {
        if !is_in_path(ZIP) {
            log_info!("Can't compress with gzip without {ZIP} executable");
            log_line!("Can't compress with gzip without {ZIP} executable");
            return Ok(());
        }
        let dest = dest.join(filename);
        let dest = dest.to_str().context("")?;
        let mut args = vec!["-r", &dest];
        args.extend(files.iter().filter_map(|file| file.to_str()));
        let _ = execute_without_output(ZIP, &args);
        Ok(())
    }

    fn gz(dest: &Path, filename: &str, files: Vec<PathBuf>) -> Result<()> {
        if !is_in_path(TAR) {
            log_info!("Can't compress with gzip without {TAR} executable");
            log_line!("Can't compress with gzip without {TAR} executable");
            return Ok(());
        }
        let dest = dest.join(filename);
        let dest = dest.to_str().context("")?;
        let mut args = vec!["-czf", &dest];
        args.extend(files.iter().filter_map(|file| file.to_str()));
        let _ = execute_without_output(TAR, &args);
        Ok(())
    }

    fn sevenz(dest: &Path, filename: &str, files: Vec<PathBuf>) -> Result<()> {
        if !is_in_path(SEVENZ) {
            log_info!("Can't compress with 7z without {SEVENZ} executable");
            log_line!("Can't compress with 7z without {SEVENZ} executable");
            return Ok(());
        }
        let dest = dest.join(filename);
        let dest = dest.to_str().context("")?;
        let mut args = vec!["a", &dest];
        args.extend(files.iter().filter_map(|file| file.to_str()));
        args.extend(&["-y", "-bd"]);
        let _ = execute_without_output(SEVENZ, &args);
        Ok(())
    }
}

impl_content!(Compresser, CompressionMethod);

impl CowStr for CompressionMethod {
    fn cow_str(&self) -> Cow<'_, str> {
        self.to_str().into()
    }
}

impl_draw_menu_with_char!(Compresser, CompressionMethod);
