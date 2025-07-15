use std::{
    fs::File,
    path::{Path, PathBuf},
    process::exit,
};

use crate::common::{tilde, CONFIG_PATH};

// TODO: comment, refactor, progress
pub fn add_plugin(path: &str) {
    // copy file
    let source = Path::new(path);
    if !source.exists() {
        eprintln!("Error installing plugin {path}. File doesn't exist.");
        exit(1);
    }
    let mut dest = PathBuf::from(tilde("~/.local/share/fm/plugins").as_ref());
    if !dest.exists() {
        if let Err(error) = std::fs::create_dir_all(&dest) {
            eprintln!("Error installing plugin {path}: {error:?}");
            exit(1);
        };
    }
    let Some(filename) = source.file_name() else {
        eprintln!("Error installing plugin {path}: couldn't extract filename");
        exit(1);
    };
    dest.push(filename);
    if let Err(error) = std::fs::copy(source, &dest) {
        eprintln!("Error installing plugin {path}: {error}");
        exit(1);
    }
    // extract filename
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
    add_to_config(&plugin_name, &dest);
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

pub fn list_plugins() {
    let config_file = File::open(tilde(CONFIG_PATH).as_ref()).expect("Couldn't open config file");
    let value: serde_yml::Value =
        serde_yml::from_reader(&config_file).expect("Couldn't read config file as yaml");

    println!("value {value:?}");

    let plugins = &value["plugins"]["previewer"];
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

fn add_to_config(plugin_name: &str, dest: &Path) {
    let config_file = File::open(tilde(CONFIG_PATH).as_ref()).expect("Couldn't open config file");
    let mut value: serde_yml::Value =
        serde_yml::from_reader(&config_file).expect("Couldn't read config file as yaml");

    // println!("value {value:?}");

    let plugins = &mut value["plugins"]["previewer"];
    if let Some(serde_yml::Mapping { ref mut map }) = plugins.as_mapping_mut() {
        println!("installed: {map:#?}");
        if !map.contains_key::<serde_yml::Value>(&plugin_name.into()) {
            map.insert(plugin_name.into(), dest.to_string_lossy().as_ref().into());
            // TODO: write preserving comments. Parse the file as a string manually
            println!("Aborting edit config since it will remove all comments from the file");
            // let config_file =
            //     File::create(tilde(CONFIG_PATH).as_ref()).expect("Couldn't open config file");
            // if let Err(e) = serde_yml::to_writer(&config_file, &value) {
            //     eprintln!("Error writing config file: {e:?}");
            // };
        } else {
            eprintln!("Error installing {plugin_name}: config files already contains a plugin with this name");
            exit(1);
        }
    }
}
