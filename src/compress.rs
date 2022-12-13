use std::fs::File;
use std::path::{Path, PathBuf};

use compress_tools::*;

use crate::fm_error::{ErrorVariant, FmError, FmResult};

pub fn decompress(source: PathBuf) -> FmResult<()> {
    let parent = source.parent().ok_or_else(|| {
        FmError::new(
            ErrorVariant::CUSTOM("decompress".to_owned()),
            "source should have a parent",
        )
    })?;
    let file = File::open(&source)?;
    Ok(uncompress_archive(&file, parent, Ownership::Preserve)?)
}

pub fn list_files<P>(source: P) -> FmResult<Vec<String>>
where
    P: AsRef<Path>,
{
    let file = File::open(source)?;
    Ok(list_archive_files(file)?)
}

// fn decompression_command(compressed_file: &Path) -> FmResult<Vec<&str>> {
//     match compressed_file
//         .extension()
//         .unwrap_or_default()
//         .to_str()
//         .unwrap_or_default()
//     {
//         "tgz" => Ok(vec!["tar", "xf"]),
//         "zip" => Ok(vec!["unzip"]),
//         "gzip" => Ok(vec!["gunzip"]),
//         "bzip2" => Ok(vec!["bunzip2"]),
//         "xz" => Ok(vec!["xz -d"]),
//         "7z" => Ok(vec!["7z", "e"]),
//         _ => Ok(vec![""]),
//     }
// }
//
// pub fn decompress(terminal: &str, compressed_file: &Path) -> FmResult<()> {
//     let mut args = decompression_command(compressed_file)?;
//     if !args.is_empty() {
//         if let Some(parent) = compressed_file.parent() {
//             let oldwd = std::env::current_dir()?;
//
//             std::env::set_current_dir(parent)?;
//             args.push(compressed_file.to_str().unwrap_or_default());
//             execute_in_child(terminal, &args)?;
//
//             std::env::set_current_dir(oldwd)?
//         } else {
//             return Ok(());
//         }
//     }
//     Ok(())
// }
