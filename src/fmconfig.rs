use std::fs::{read_dir, remove_file};
use std::process::exit;

use anyhow::Result;
use clap::{Args as ClapArgs, Parser, Subcommand};

use fm::app::FM;
use fm::common::{CONFIG_PATH, TMP_THUMBNAILS_DIR};
use fm::config::{cloud_config, load_config, Config};
use fm::io::{add_plugin, list_plugins, remove_plugin};

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
/// FM : dired / ranger like file manager{n} {n}Config files: ~/.config/fm/{n}Documentation: <https://github.com/qkzk/fm>{n}fmconfig is fm configuration tool{n}
pub struct Args {
    #[command(subcommand)]
    pub plugin: Option<PluginCommand>,

    #[clap(flatten)]
    pub run_args: RunArgs,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct RunArgs {
    /// Print keybinds
    #[arg(long, default_value_t = false)]
    pub keybinds: bool,

    /// Configure a google drive client
    #[arg(long, default_value_t = false)]
    pub cloudconfig: bool,

    /// Clear the video thumbnail cache
    #[arg(long, default_value_t = false)]
    pub clear_cache: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommand {
    /// Plugin management. fm plugin -h for more details.
    Plugin {
        #[command(subcommand)]
        action: PluginSubCommand,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum PluginSubCommand {
    /// Add an already compiled plugin from a path to .so file
    Add { path: String },
    /// Remove a plugin by name
    Remove { name: String },
    /// List all installed plugins
    List,
}

fn exit_with_cloud_config() -> Result<()> {
    cloud_config()?;
    exit(0);
}

fn exit_with_clear_cache() -> Result<()> {
    read_dir(TMP_THUMBNAILS_DIR)?
        .filter_map(|entry| entry.ok())
        .for_each(|e| {
            if let Err(e) = remove_file(e.path()) {
                println!("Couldn't remove {TMP_THUMBNAILS_DIR}: error {e}");
            }
        });
    println!("Cleared {TMP_THUMBNAILS_DIR}");
    exit(0);
}

fn exit_with_binds(config: &Config) -> ! {
    println!("{binds}", binds = config.binds.to_str());
    exit(0);
}

fn exit_manage_plugins(plugin: &PluginCommand) -> ! {
    let PluginCommand::Plugin { action } = plugin;
    match action {
        PluginSubCommand::Add { path } => add_plugin(path),
        PluginSubCommand::Remove { name } => remove_plugin(name),
        PluginSubCommand::List => list_plugins(),
    }
    exit(0);
}

fn main() -> Result<()> {
    println!("Welcome to Fm configuration application.");
    let args = Args::parse();
    if args.run_args.cloudconfig {
        exit_with_cloud_config()?;
    }
    if args.run_args.clear_cache {
        exit_with_clear_cache()?;
    }
    if let Some(plugin) = args.plugin {
        exit_manage_plugins(&plugin);
    }
    if args.run_args.keybinds {
        let Ok(config) = load_config(CONFIG_PATH) else {
            FM::exit_wrong_config()
        };
        exit_with_binds(&config);
    }
    println!("Bye !");
    Ok(())
}
