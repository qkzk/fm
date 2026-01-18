use std::borrow::Borrow;
use std::io::{prelude::*, Write};

/// fm needs its configuration files when running. This build will ensure that
/// the default config files are copied where they should be and the default versions are
/// saved in source code.
/// Archive the config_files directory to a small zip file (~4kB) which will be included in source code.
/// It's used to create default config files if no config file is found at runtime.
/// As a second effect, the user can erase its config files if they're messed up.
///
/// Creates a config folder for fm with default config files.
/// The source is `config_files/fm`.
/// The destination is `~/.config/fm`.
/// If there's already some configuration files, no overwrite is done.
fn main() -> std::io::Result<()> {
    zip_default_config()?;
    copy_config_to_home_dir();
    Ok(())
}

fn zip_default_config() -> std::io::Result<()> {
    let config_files = get_config_paths()?;
    let archive = std::fs::File::create("config_files/fm_config.zip")?;
    zip(archive, config_files)
}

fn get_config_paths() -> std::io::Result<Vec<std::path::PathBuf>> {
    let config_path = std::path::PathBuf::from("config_files/fm");
    let mut stack = vec![config_path];
    let mut collection = vec![];

    while let Some(current) = stack.pop() {
        for dir_entry in std::fs::read_dir(current)? {
            let path = dir_entry?.path();
            if path.is_dir() {
                stack.push(path)
            } else {
                collection.push(path)
            }
        }
    }
    Ok(collection)
}

fn zip(archive: std::fs::File, files: Vec<std::path::PathBuf>) -> std::io::Result<()> {
    let mut zip = zip::ZipWriter::new(archive);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o755)
        .compression_method(zip::CompressionMethod::Bzip2);
    for file in files.iter() {
        let Ok(start_file) = file.strip_prefix("config_files/fm/") else {
            continue;
        };
        zip.start_file(start_file.to_string_lossy().as_ref(), options)?;
        let mut buffer = Vec::new();
        let mut content = std::fs::File::open(file)?;
        content.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;
    }

    // Finish zip file
    zip.finish()?;
    Ok(())
}

fn copy_config_to_home_dir() {
    let Ok(mut default_config_files) = std::env::current_dir() else {
        eprintln!("Environment variable $PWD should be set. Couldn't find the source folder.");
        return;
    };
    default_config_files.push("config_files/fm");

    let config_folder_cow = tilde("~/.config");
    let config_folder: &str = config_folder_cow.borrow();
    let mut copy_options = fs_extra::dir::CopyOptions::new();
    copy_options.skip_exist = true;

    match fs_extra::dir::copy(default_config_files, config_folder, &copy_options) {
        Ok(_) => (),
        Err(e) => eprintln!("{e:?}"),
    }
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .and_then(|h| if h.is_empty() { None } else { Some(h) })
        .map(std::path::PathBuf::from)
}

/// Expand ~/Downloads to /home/user/Downloads where user is the current user.
/// Copied from <https://gitlab.com/ijackson/rust-shellexpand/-/blob/main/src/funcs.rs?ref_type=heads#L673>
fn tilde(input_str: &str) -> std::borrow::Cow<'_, str> {
    if let Some(input_after_tilde) = input_str.strip_prefix('~') {
        if input_after_tilde.is_empty() || input_after_tilde.starts_with('/') {
            if let Some(hd) = home_dir() {
                let result = format!("{}{}", hd.display(), input_after_tilde);
                result.into()
            } else {
                // home dir is not available
                input_str.into()
            }
        } else {
            // we cannot handle `~otheruser/` paths yet
            input_str.into()
        }
    } else {
        // input doesn't start with tilde
        input_str.into()
    }
}
