use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
/// FM : dired / ranger like file manager{n}
pub struct Args {
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
}
