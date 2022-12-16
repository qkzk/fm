use std::borrow::Borrow;

/// Creates a config folder for fm with default config files.
/// The source is `config_files/fm`.
/// The destination is `~/.config/fm`.
fn main() {
    let mut default_config_files = std::env::current_dir().unwrap();
    default_config_files.push("config_files/fm");

    let config_folder_cow = shellexpand::tilde("~/.config");
    let config_folder: &str = &config_folder_cow.borrow();

    fs_extra::dir::copy(
        default_config_files,
        config_folder,
        &fs_extra::dir::CopyOptions::new(),
    )
    .unwrap();
}
