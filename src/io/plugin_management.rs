use std::{
    fs::File,
    path::{Path, PathBuf},
    process::exit,
};

use crate::common::{tilde, CONFIG_PATH};

/// Add a plugin from a lib .so file.
/// Copy the plugin to "~/.local/share/fm/plugins/"
/// Add "plugin_name: adress" in config file, as first element.
///
/// Warn the user and exit the process with error code 1 if any error occurs.
///
/// It should never crash.
pub fn add_plugin(path: &str) {
    println!("Installing {path}...");
    let source = Path::new(path);
    if !source.exists() {
        eprintln!("Error installing plugin {path}. File doesn't exist.");
        exit(1);
    }
    println!("Found lib source file...");
    let dest = build_dest_path(source);
    copy_source_to_dest(source, &dest);
    println!("Copied source file to {dest}...", dest = dest.display());
    let plugin_name = get_plugin_name(source);
    add_to_config(&plugin_name, &dest);
    println!(
        "Added {plugin_name}: {dest} to config.",
        dest = dest.display()
    );
    println!("Installation done.");
}

fn build_dest_path(source: &Path) -> PathBuf {
    let mut dest = PathBuf::from(tilde("~/.local/share/fm/plugins").as_ref());
    if !dest.exists() {
        if let Err(error) = std::fs::create_dir_all(&dest) {
            eprintln!("Error: {error:?}");
            exit(1);
        };
    }
    let Some(filename) = source.file_name() else {
        eprintln!("Error: couldn't extract filename");
        exit(1);
    };
    dest.push(filename);
    dest
}

fn copy_source_to_dest(source: &Path, dest: &Path) {
    if let Err(error) = std::fs::copy(source, dest) {
        eprintln!("Error: {error}");
        exit(1);
    }
}

fn get_plugin_name(source: &Path) -> String {
    let filename = source.file_name().expect("source should have a filename");
    let mut plugin_name = filename.to_string_lossy().to_string();
    if plugin_name.starts_with("lib") {
        plugin_name = plugin_name
            .strip_prefix("lib")
            .expect("Should start with lib")
            .to_owned();
    }
    if plugin_name.ends_with(".so") {
        plugin_name = plugin_name
            .strip_suffix(".so")
            .expect("Should end with .so")
            .to_owned();
    }
    plugin_name
}

// TODO: write the code..
pub fn remove_plugin(name: &str) {
    // find the plugin in config file -> get its path.
    // delete the path
    // edit the config file content.
    // write the config file
    todo!();
}

// TODO: write the code
pub fn download_plugin(url: &str) {
    // is it a github repo or a .so file ?
    // if repo {
    //      clone url
    //      compile
    //      get path to .so
    // }
    // else {
    //  path = download(url)
    // }
    // call add_plugin(path)
    todo!();
}

/// List all installed plugins.
///
/// Warn the user and exit if any error occurs.
pub fn list_plugins() {
    let config_file = File::open(config_path()).expect("Couldn't open config file");
    let value: serde_yml::Value =
        serde_yml::from_reader(&config_file).expect("Couldn't read config file as yaml");

    let plugins = &value["plugins"]["previewer"];
    println!("Installed plugins:");
    if let Some(serde_yml::Mapping { ref map }) = plugins.as_mapping() {
        for (name, path) in map.iter() {
            let Some(name) = name.as_str() else {
                eprintln!("Couldn't parse name as string");
                return;
            };
            let Some(path) = path.as_str() else {
                eprintln!("Couldn't parse path as string");
                return;
            };
            let exists = if Path::new(path).exists() { "ok" } else { "??" };
            println!("[{exists}]: {name}: {path}");
        }
    }
}

fn config_path() -> String {
    tilde(CONFIG_PATH).to_string()
}

fn add_to_config(plugin_name: &str, dest: &Path) {
    let config_path = config_path();
    if is_plugin_name_in_config(&config_path, plugin_name) {
        eprintln!("Config files already contains a plugin with this name");
        exit(1);
    }
    add_lib_to_config(&config_path, plugin_name, dest);
}

fn is_plugin_name_in_config(config_path: &str, plugin_name: &str) -> bool {
    let config_file = File::open(config_path).expect("Couldn't open config file");
    let config_values: serde_yml::Value =
        serde_yml::from_reader(&config_file).expect("Couldn't read config file as yaml");
    let plugins = &config_values["plugins"]["previewer"];
    if let Some(serde_yml::Mapping { ref map }) = plugins.as_mapping() {
        map.contains_key::<serde_yml::Value>(&plugin_name.into())
    } else {
        false
    }
}

fn add_lib_to_config(config_path: &str, plugin_name: &str, dest: &Path) {
    let config_content = std::fs::read_to_string(&config_path).expect("Couldn't read config file");
    let mut lines: Vec<_> = config_content.lines().map(|l| l.to_string()).collect();
    let mut dest_index = None;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("plugins:") && lines[index + 1].starts_with("  previewer:") {
            println!("found {index} {line}");
            dest_index = Some(index + 2);
            break;
        }
    }
    if let Some(index) = dest_index {
        let new_line = format!("    '{plugin_name}': \"{d}\"", d = dest.display());
        if index >= lines.len() {
            lines.push(new_line)
        } else {
            lines.insert(index, new_line)
        }
    }
    let new_content = lines.join("\n");
    if let Err(e) = std::fs::write(config_path, new_content) {
        eprintln!("Error installing {plugin_name}. Couldn't write to config file: {e:?}");
        exit(1);
    }
}
