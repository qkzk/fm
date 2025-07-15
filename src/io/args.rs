use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
/// FM : dired / ranger like file manager{n} {n}Config files: ~/.config/fm/{n}Documentation: <https://github.com/qkzk/fm>{n}
pub struct Args {
    #[command(subcommand)]
    pub plugin: Option<PluginCommand>,

    #[clap(flatten)]
    pub run_args: RunArgs,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct RunArgs {
    /// Starting path. directory or file
    #[arg(short, long, default_value_t = String::from("."))]
    pub path: String,

    /// Nvim server
    #[arg(short, long, default_value_t = String::from(""))]
    pub server: String,

    /// Display all files (hidden)
    #[arg(short = 'A', long, default_value_t = false)]
    pub all: bool,

    /// Enable logging
    #[arg(short = 'l', long, default_value_t = false)]
    pub log: bool,

    /// Started inside neovim terminal emulator
    #[arg(long, default_value_t = false)]
    pub neovim: bool,

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
    /// Download a plugin from a URL
    Download { url: String },
    /// List all installed plugins
    List,
}
