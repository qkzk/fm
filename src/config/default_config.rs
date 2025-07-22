use crate::{
    common::{tilde, CONFIG_FOLDER},
    modes::decompress_zip,
};

/// Creates the default config if it doesn't exists.
/// Creates the trash folder if it doesn't exists.
///
/// Errors
///
/// It may fail if the user has no write access to $HOME which shouldn't happen in a normal environment.
pub fn make_default_config_files() -> std::io::Result<()> {
    create_config_folder()?;
    copy_default_config_files()?;
    create_trash_folders()?;
    Ok(())
}

/// Creates the config folder in ~/.config/fm
fn create_config_folder() -> std::io::Result<()> {
    let p = tilde(CONFIG_FOLDER);
    std::fs::create_dir_all(p.as_ref())
}

/// Copy the config files to ~/.config/fm/
/// The default config files are zipped and included in the code. I couldn't find a better idea...
/// It uses ~120 bytes.
/// Once copied, the zip file in unzipped and removed.
fn copy_default_config_files() -> std::io::Result<()> {
    // TODO automatise the zipping
    let mut dest = std::path::PathBuf::from(tilde(CONFIG_FOLDER).as_ref());
    dest.push("fm_config.zip");
    let config_bytes = include_bytes!("../../config_files/fm_config.zip");

    std::fs::write(&dest, config_bytes)?;
    decompress_zip(&dest)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Couldn't decompress"))?;
    std::fs::remove_file(&dest)
}

/// Creates the trash folders:
///  ~.local
///     |- Trash
///          |- expunged/
///          |- files/
///          |- info/
fn create_trash_folders() -> std::io::Result<()> {
    for dir in &[
        "~/.local/share/Trash/expunged",
        "~/.local/share/Trash/files",
        "~/.local/share/Trash/info",
    ] {
        std::fs::create_dir_all(tilde(dir).as_ref())?;
    }
    Ok(())
}
