use std::borrow::Borrow;

/// Creates a config folder for fm with default config files.
/// The source is `config_files/fm`.
/// The destination is `~/.config/fm`.
/// If there's already some configuration files, no overwrite is done.
fn main() {
    let Ok(mut default_config_files) = std::env::current_dir() else { 
        eprintln!("Environment variable $PWD should be set. Couldn't find the source folder.");
        return
    };
    default_config_files.push("config_files/fm");

    let config_folder_cow = shellexpand::tilde("~/.config");
    let config_folder: &str = config_folder_cow.borrow();
    let mut copy_options = fs_extra::dir::CopyOptions::new();
    copy_options.skip_exist = true;

    match fs_extra::dir::copy(default_config_files, config_folder, &copy_options) {
        Ok(_) => (),
        Err(e) => eprintln!("{e:?}"),
    }
}
