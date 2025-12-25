use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
    process::exit,
};

use serde_yaml_ng::{from_reader, from_value, Value};

use crate::common::{tilde, CONFIG_PATH, PLUGIN_LIBSO_PATH, REPOSITORIES_PATH};
use crate::io::execute_and_capture_output_with_path;

// TODO: allow for other sources than github with a complete url.

/// Install a plugin from its github repository.
/// It can also be used to update a plugin if its repository was updated.
///
/// # Usage
///
/// `fmconfig plugin install qkzk/bat_previewer`
///
/// # Steps
///
/// It will successively perform:
/// 1. parse the author and plugin name from `author_plugin`, spliting it at the first /
/// 2. build the repository address which is a temporary folder located at  [`crate::common::REPOSITORIES_PATH`]
/// 3. clone the repository in the temporary folder
/// 4. build the release libso file of the plugin
/// 5. copy the libso file to its destination [`crate::common::PLUGIN_LIBSO_PATH`] and check the result of compilation
/// 6. add the plugin to the config file in [`crate::common::CONFIG_PATH`]
/// 7. remove the repository folder from temporary files
///
/// # Failure
///
/// If any step fails (except adding the plugin to config file), it exits with an error printed to stderr.
pub fn install_plugin(author_plugin: &str) {
    println!("Installing {author_plugin} from its github repository");
    // 1. Parse author & plugin name
    let plugin = parse_plugin_name(author_plugin);
    // 2. build repository address
    let repositories_path = tilde(REPOSITORIES_PATH);
    let repositories_path = Path::new(repositories_path.as_ref());
    // 3. Create clone dir
    create_clone_directory(repositories_path);
    // 4. git clone
    clone_repository(repositories_path, author_plugin);
    // 5. build release target
    let plugin_path = cargo_build_release(repositories_path, plugin);
    // 6. check compilation process
    let Some(libso_path) = find_compiled_target(plugin_path, plugin) else {
        remove_repo_directory().expect("Couldn't delete plugin repository");
        exit(6);
    };
    if _add_plugin(&libso_path) {
        // 7. remove plugin repository from ~/.local/share/fm/
        remove_repo_directory().expect("Couldn't delete plugin repository");
        println!("Installation done");
    }
}

/// Split `author_plugin` and returns the plugin name.
/// `"qkzk/bat_previewer"` -> `"bat_previewer"`
///
/// # Failure
/// prints error to stderr and exits with
/// - error code 2 if the `author_plugin` doesn't contain a /
/// - error code 3 if `author_plugin` contains nothing after /
fn parse_plugin_name(author_plugin: &str) -> &str {
    let mut split = author_plugin.split('/');
    let Some(_) = split.next() else {
        eprintln!(
            "Error installing plugin {author_plugin} isn't valid. Please use author/plugin format."
        );
        exit(2);
    };
    let Some(plugin) = split.next() else {
        eprintln!(
            "Error installing plugin {author_plugin} isn't valid. Please use author/plugin format."
        );
        exit(3);
    };
    plugin
}

/// Creates the [`REPOSITORIES_PATH`] directory
///
/// # Failure
/// Exits with error code 3 if the directory can't be created.
fn create_clone_directory(repositories_path: &Path) {
    match std::fs::create_dir_all(repositories_path) {
        Ok(()) => println!(
            "- Created {repositories_path}",
            repositories_path = repositories_path.display()
        ),
        Err(error) => {
            eprintln!("Error creating directories for repostories: {error:?}");
            exit(3);
        }
    }
}

/// Clone the plugin repository.
/// Executes "git clone --depth 1 git@github.com:{author_plugin}.git" from [`crate::common::REPOSITORIES_PATH`]
///
/// # Failure
/// Exits with error code 4 if the clone failed, printing the error to stderr.
fn clone_repository(plugin_repositories: &Path, author_plugin: &str) {
    let args = [
        "clone",
        "--depth",
        "1",
        &format!("git@github.com:{author_plugin}.git"),
    ];
    let output = execute_and_capture_output_with_path("git", plugin_repositories, &args);
    match output {
        Ok(stdout) => println!("- Cloned {author_plugin}{stdout} git repository"),
        Err(stderr) => {
            eprintln!("Error cloning the repository :");
            eprintln!("{}", stderr);
            let _ = remove_repo_directory();
            exit(4);
        }
    }
}

/// Build the libso file.
/// Executes `cargo build --release` from `plugin_path` which should be a subdirectory of `/tmp/fm/repositories/` called `plugin`.
/// Returns the builded libso file path.
///
/// # Failure
/// Exits with error code 5 if the compilation failed.
fn cargo_build_release(plugin_path: &Path, plugin: &str) -> PathBuf {
    // 4. cargo build --release
    let args = ["build", "--release"];
    let mut plugin_path = plugin_path.to_path_buf();
    plugin_path.push(plugin);
    let output = execute_and_capture_output_with_path("cargo", &plugin_path, &args);
    match output {
        Ok(stdout) => {
            println!("- Compiled plugin {plugin} libso file");
            if !stdout.is_empty() {
                println!("- {stdout}")
            }
        }
        Err(stderr) => {
            eprintln!("Error compiling the plugin :");
            eprintln!("{}", stderr);
            remove_repo_directory().expect("Couldn't delete plugin repository");
            exit(5);
        }
    }
    plugin_path
}

/// Find the compilation target from plugin_path and returns its full path.
fn find_compiled_target(mut plugin_path: PathBuf, plugin: &str) -> Option<PathBuf> {
    let ext = format!("target/release/lib{plugin}.so");
    plugin_path.push(ext);
    if plugin_path.exists() {
        Some(plugin_path)
    } else {
        None
    }
}

/// Add a plugin from a libso file path.
///
/// # Steps
/// 1. copy the plugin to [`crate::common::PLUGIN_LIBSO_PATH`].
/// 2. add "`plugin_name: libso_file_path`" in config file, as first element.
///
/// # Failure
///
/// Warn the user and exit the process with various error codes if any error occurs.
///
/// It should never crash.
pub fn add_plugin<P>(path: P)
where
    P: AsRef<Path>,
{
    println!("Installing {path}...", path = path.as_ref().display());
    if _add_plugin(&path) {
        println!(
            "Plugin {path} added to configuration file.",
            path = path.as_ref().display()
        );
    } else {
        eprintln!(
            "Something went wrong installing {path}.",
            path = path.as_ref().display()
        );
        exit(1);
    }
}

/// Internal adding a plugin.
/// This functions exists to allow more precised messages.
/// All the work is done here.
/// See [crate::io::add_plugin] for more details.
pub fn _add_plugin<P>(path: P) -> bool
where
    P: AsRef<Path>,
{
    let source = path.as_ref();
    if !source.exists() {
        eprintln!(
            "Error installing plugin {path}. File doesn't exist.",
            path = path.as_ref().display()
        );
        exit(1);
    }
    let dest = build_libso_destination_path(source);
    copy_source_to_dest(source, &dest);
    println!("- Copied libso file to {dest}", dest = dest.display());
    let plugin_name = get_plugin_name(source);
    add_to_config(&plugin_name, &dest);
    println!(
        "- Added {plugin_name}: {dest} to config file.",
        dest = dest.display()
    );
    true
}

/// Build the destination filepath from the source libso file.
/// It will be located in [`crate::common::PLUGIN_LIBSO_PATH`].
///
/// # Failure
/// Exists with error code 1 if the path doesn't exist.
fn build_libso_destination_path(source: &Path) -> PathBuf {
    let mut dest = PathBuf::from(tilde(PLUGIN_LIBSO_PATH).as_ref());
    if let Err(error) = std::fs::create_dir_all(&dest) {
        eprintln!("Couldn't create {PLUGIN_LIBSO_PATH}");
        eprintln!("Error: {error:?}");
        exit(1);
    };

    let Some(filename) = source.file_name() else {
        eprintln!("Error: couldn't extract filename");
        exit(1);
    };
    dest.push(filename);
    dest
}

/// Copy the libso file to its destination.
///
/// # Failure
/// Exists with error code 1 if the file can't be copied.
fn copy_source_to_dest(source: &Path, dest: &Path) {
    if let Err(error) = std::fs::copy(source, dest) {
        eprintln!("Error copying the libsofile: {error}");
        exit(1);
    }
}

/// Get the plugin name from ist libso file path.
/// ~/.local/shared/fm/plugins/libbat_previewer.so -> bat_previewer.
///
/// # Failure
/// It will crash if `source` doesn't start with `"lib"` or doesn't end with `".so"`.
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
/// Plugin lib.so file will be deleted and removed from config file
///
/// # Steps
///
/// 1. remove the repository folder if it exists
/// 2. remove the libso file from [`crate::common::REPOSITORIES_PATH`]
/// 3. remove the `plugin_name: plugin_libso_path` from [`crate::common::CONFIG_PATH`]
///
/// # Failure
///
/// If something went wrong it will exit with an error message printed to stderr.
pub fn remove_plugin(removed_name: &str) {
    let _ = remove_repo_directory();
    remove_libso_file(removed_name);
    remove_lib_from_config(&config_path(), removed_name);
}

/// Remove the repository folder located at [`crate::common::REPOSITORIES_PATH`].
///
/// # Failure
/// Exits with code 2 if the repository couldn't be removed.
fn remove_repo_directory() -> std::io::Result<()> {
    match std::fs::remove_dir_all(REPOSITORIES_PATH) {
        Ok(()) => {
            println!("- Removed repository");
            Ok(())
        }
        Err(err) => {
            eprintln!("Coudln't remove repository: {err:?}",);
            Err(err)
        }
    }
}

/// Remove the libso file of `removed_name`.
/// If the plugin was installed with `fmconfig plugin install author/plugin` it should be located at `~/.local/share/fm/plugins/lib{removed_name}.so`.
///
/// # Failure
/// Exits with code 2 if the file couldn't be removed.
fn remove_libso_file(removed_name: &str) {
    let mut found_in_config = false;
    for (installed_name, path, exist) in list_plugins_details().iter() {
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

/// List all installed plugins referenced in `[crate::common::CONFIG_PATH]`
/// It will print a list of `status`, plugin name and libso paths to stdout.
///
/// `status` can either be:
/// - `ok` if the libso file exists or
/// - `??` if it doesn't.
///
/// # Warning
///
/// It doesn't check if the plugin works, only the existence of its libso file.
///
/// # Error
///
/// It may fail if the config file isn't formated properly.
pub fn list_plugins() {
    println!("Installed plugins:");
    for (name, path, exist) in list_plugins_details().iter() {
        let exists = if *exist { "ok" } else { "??" };
        println!("[{exists}]: {name}: {path}");
    }
}

/// Returns a vector of  `(name, path, existance)` for each referenced plugin in config file.
fn list_plugins_details() -> Vec<(String, String, bool)> {
    let config_file = File::open(config_path().as_ref()).expect("Couldn't open config file");
    let mut installed = vec![];

    let config_values: Value =
        from_reader(&config_file).expect("Couldn't read config file as yaml");
    let plugins = &config_values["plugins"]["previewer"];
    let Ok(dmap) = from_value::<BTreeMap<String, String>>(plugins.to_owned()) else {
        return vec![];
    };
    for (plugin, path) in dmap.into_iter() {
        let exists = Path::new(&path).exists();
        installed.push((plugin, path, exists))
    }
    installed
}

/// Build the config_file path from [`crate::common::CONFIG_PATH`].
fn config_path() -> Cow<'static, str> {
    tilde(CONFIG_PATH)
}

/// Add the plugin to config file. Does nothing if the plugin is already there.
fn add_to_config(plugin_name: &str, dest: &Path) {
    let config_path = config_path();
    if is_plugin_name_in_config(&config_path, plugin_name) {
        println!("- Config file {config_path} already contains a plugin called \"{plugin_name}\"");
        return;
    }
    add_lib_to_config(&config_path, plugin_name, dest);
}

/// True iff the plugin is referenced in the config file.
fn is_plugin_name_in_config(config_path: &str, plugin_name: &str) -> bool {
    let config_file = File::open(config_path).expect("Couldn't open config file");
    let config_values: Value =
        from_reader(&config_file).expect("Couldn't read config file as yaml");
    let plugins = &config_values["plugins"]["previewer"];
    let Ok(dmap) = from_value::<BTreeMap<String, String>>(plugins.to_owned()) else {
        return false;
    };
    dmap.contains_key(plugin_name)
}

/// Writes the config file to the config file.
/// Expects the file to not have the plugin name already.
///
/// # Failure
/// Will crash if the config can't be read
/// or if it can't be written (error code 1),
/// or if the `plugin:previewer:` part can't be found (error code 2).
fn add_lib_to_config(config_path: &str, plugin_name: &str, dest: &Path) {
    let mut lines = extract_config_lines(config_path);
    let new_line = format!("    '{plugin_name}': \"{d}\"", d = dest.display());

    complete_lines_with_required_parts(&mut lines, new_line);

    let new_content = lines.join("\n");
    if let Err(e) = std::fs::write(config_path, new_content) {
        eprintln!("Error installing {plugin_name}. Couldn't write to config file: {e:?}");
        exit(1);
    }
}

/// Ensures new plugins are inserted AFTER the `plugins:previewer:` mapping.
/// If no such mapping is found in configuration, we add it at the end of the file before inserting the new plugin.
/// We only cover for the cases where:
/// - `plugins:previewer:` doesn't contain the plugin,
/// - `plugins:previewer:` is empty,
/// - `plugins:` is empty,
/// - `there's no plugins:` in config file.
///
/// So, the strange case where previewer: is present and plugin: isn't covered.
fn complete_lines_with_required_parts(lines: &mut Vec<String>, new_line: String) {
    match find_dest_index(lines) {
        Some(index) => {
            if index >= lines.len() {
                lines.push(new_line)
            } else {
                lines.insert(index, new_line)
            }
        }
        None => {
            if lines.iter().all(|s| s != "plugins:") {
                lines.push("plugins:".to_string());
            }
            lines.push("  previewer:".to_string());
            lines.push(new_line);
        }
    }
}

/// Read the config and returns its content as a vector of lines.
/// Expect `config_path` to be an expanded full path like `/home/user/.config/fm` and not `~/.config.fm`.
fn extract_config_lines(config_path: &str) -> Vec<String> {
    let config_content = std::fs::read_to_string(config_path).expect("Couldn't read config file");
    config_content
        .lines()
        .map(|line| line.to_string())
        .collect()
}

/// Returns the index of the `plugin: previewer:` section for its content.
fn find_dest_index(lines: &[String]) -> Option<usize> {
    // println!("{least_before}", least_before = lines[lines.len() - 2]);
    // println!("{least}", least = lines[lines.len() - 1]);
    for (plugin_index, line) in lines.iter().enumerate() {
        if line.starts_with("plugins:") {
            for (previewer_index, line) in lines.iter().enumerate().skip(plugin_index) {
                if line.starts_with("  previewer:") {
                    return Some(previewer_index + 1);
                }
            }
            break;
        }
    }
    None
}

/// Remove the libso file from the config file.
/// Expect `config_path` to be an expanded full path like `/home/user/.config/fm` and not `~/.config.fm`.
///
/// # Failure
/// Will crash if the config can't be read
/// or if it can't be written (error code 1),
/// or if the `plugin:previewer:` part can't be found (error code 2).
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
