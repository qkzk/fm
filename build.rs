use std::borrow::Borrow;

/// Creates a config folder for fm with default config files.
/// The source is `config_files/fm`.
/// The destination is `~/.config/fm`.
/// If there's already some configuration files, no overwrite is done.
fn main() {
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

    update_breaking_config()
}

/// Remove old binds from user config file.
///
/// Remove all binds to `Jump` and `Mocp...` variants since they were removed from fm.
fn update_breaking_config() {
    let config = tilde("~/.config/fm/config.yaml");
    let config: &str = config.borrow();
    let content = std::fs::read_to_string(config)
        .expect("config file should be readable")
        .lines()
        .map(String::from)
        .filter(|line| !line.contains("Jump"))
        .filter(|line| !line.contains("Mocp"))
        .collect::<Vec<String>>()
        .join("\n");
    std::fs::write(config, content).expect("config should be writabe");
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .and_then(|h| if h.is_empty() { None } else { Some(h) })
        .map(std::path::PathBuf::from)
}

/// Expand ~/Downloads to /home/user/Downloads where user is the current user.
/// Copied from <https://gitlab.com/ijackson/rust-shellexpand/-/blob/main/src/funcs.rs?ref_type=heads#L673>
fn tilde(input_str: &str) -> std::borrow::Cow<str> {
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
