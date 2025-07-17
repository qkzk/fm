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

/// Remove a plugin by its name.
/// Plugin lib.so file will be deleted and erased from config file
/// find the plugin in config file -> get its path.
/// delete the path
/// edit the config file content.
/// write the config file
pub fn remove_plugin(removed_name: &str) {
    remove_libso_file(removed_name);
    remove_lib_from_config(&config_path(), removed_name);
}

fn remove_libso_file(removed_name: &str) {
    let mut found_in_config = false;
    for (installed_name, path, exist) in list_plugins_pairs().iter() {
        if installed_name == removed_name && *exist {
            found_in_config = true;
            match std::fs::remove_file(path) {
                Ok(()) => println!("Removed {path}"),
                Err(e) => eprintln!("Couldn't remove {path}: {e:?}"),
            };
        }
    }
    if !found_in_config {
        eprintln!("Didn't find {removed_name} in config file. Run `fm plugin list` to see installed plugins.");
        exit(1);
    }
}

/// List all installed plugins.
///
/// Warn the user and exit if any error occurs.
pub fn list_plugins() {
    println!("Installed plugins:");
    for (name, path, exist) in list_plugins_pairs().iter() {
        let exists = if *exist { "ok" } else { "??" };
        println!("[{exists}]: {name}: {path}");
    }
}

fn list_plugins_pairs() -> Vec<(String, String, bool)> {
    let config_file = File::open(config_path()).expect("Couldn't open config file");
    let value: serde_yml::Value =
        serde_yml::from_reader(&config_file).expect("Couldn't read config file as yaml");

    let plugins = &value["plugins"]["previewer"];
    let mut installed = vec![];
    if let Some(serde_yml::Mapping { ref map }) = plugins.as_mapping() {
        for (name, path) in map.iter() {
            let Some(name) = name.as_str() else {
                continue;
            };
            let Some(path) = path.as_str() else {
                continue;
            };
            let exists = Path::new(path).exists();
            installed.push((name.to_owned(), path.to_owned(), exists));
        }
    }
    installed
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
    let config_content = std::fs::read_to_string(config_path).expect("Couldn't read config file");
    let mut lines: Vec<_> = config_content.lines().map(|l| l.to_string()).collect();
    let mut dest_index = None;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("plugins:") && lines[index + 1].starts_with("  previewer:") {
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

fn remove_lib_from_config(config_path: &str, plugin_name: &str) {
    let config_content = std::fs::read_to_string(config_path).expect("Couldn't read config file");
    let mut lines: Vec<_> = config_content.lines().map(|l| l.to_string()).collect();
    for index in 0..lines.len() {
        let line = &lines[index];
        if line.starts_with(&format!("    '{plugin_name}': ",)) {
            lines.remove(index);
            break;
        }
    }
    let new_content = lines.join("\n");
    match std::fs::write(config_path, new_content) {
        Ok(()) => println!("Removed {plugin_name} from config file"),
        Err(e) => {
            eprintln!("Error removing {plugin_name}. Couldn't write to config file: {e:?}");
            exit(1);
        }
    }
}
