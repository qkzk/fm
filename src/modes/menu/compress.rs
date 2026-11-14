use std::borrow::Cow;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::common::{filename_from_path, is_in_path, path_to_string, GZIP, SEVENZ, TAR, XZ, ZIP};
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
        let Some(selected_method) = self.selected() else {
            return Ok(());
        };
        let base_name = Self::base_name(&files)?.to_string();
        let single = Self::is_single_normal_file(&files);
        match selected_method {
            #[rustfmt::skip]
            CompressionMethod::Zip            => Self::zip(here, base_name + ".zip", files)?,
            #[rustfmt::skip]
            CompressionMethod::Zlib if single => Self::simple(&files[0], XZ)?,
            #[rustfmt::skip]
            CompressionMethod::Zlib           => Self::tar(here, base_name + ".tar.xz", files, "xz", "-cJf")?,
            #[rustfmt::skip]
            CompressionMethod::Gz if single   => Self::simple(&files[0], GZIP)?,
            #[rustfmt::skip]
            CompressionMethod::Gz             => Self::tar(here, base_name + ".tar.gz", files, "gzip", "-czf")?,
            #[rustfmt::skip]
            CompressionMethod::Sevenz         => Self::sevenz(here, base_name + ".7z", files)?,
        }
        log_line!(
            "Compressed into {selected_method}",
            selected_method = selected_method.to_str()
        );
        Ok(())
    }

    fn is_single_normal_file(files: &[PathBuf]) -> bool {
        if files.len() != 1 {
            false
        } else {
            files[0].is_file()
        }
    }

    fn base_name(files: &[PathBuf]) -> Result<&str> {
        if files.len() == 1 {
            filename_from_path(&files[0])
        } else {
            Ok("archive")
        }
    }

    fn zip(dest: &Path, filename: String, files: Vec<PathBuf>) -> Result<()> {
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

    fn simple(file: &Path, exe: &str) -> Result<()> {
        if !is_in_path(exe) {
            log_info!("Can't compress with xz without {exe} executable");
            log_line!("Can't compress with xz without {exe} executable");
            return Ok(());
        }
        let p = path_to_string(&file);
        let args = vec!["-k", &p];
        let _ = execute_without_output(exe, &args);
        Ok(())
    }

    fn tar(
        dest: &Path,
        filename: String,
        files: Vec<PathBuf>,
        method: &str,
        flags: &str,
    ) -> Result<()> {
        if !is_in_path(TAR) {
            log_info!("Can't compress with {method} without {TAR} executable");
            log_line!("Can't compress with {method} without {TAR} executable");
            return Ok(());
        }
        let dest = dest.join(filename);
        let dest = dest.to_str().context("")?;
        let mut args = vec![flags, &dest];
        args.extend(files.iter().filter_map(|file| file.to_str()));
        let _ = execute_without_output(TAR, &args);
        Ok(())
    }

    fn sevenz(dest: &Path, filename: String, files: Vec<PathBuf>) -> Result<()> {
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
