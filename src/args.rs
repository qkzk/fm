use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
/// FM : dired like file manager{n}
pub struct Args {
    /// Starting path. directory or file
    #[arg(short, long, default_value_t = String::from("."))]
    pub path: String,

    /// Nvim server
    #[arg(short, long, default_value_t = String::from(""))]
    pub server: String,

    /// Dual pane ? default to true
    #[arg(short, long, default_value_t = false)]
    pub dual: bool,

    /// Display file metadata ? default to true
    #[arg(short, long, default_value_t = false)]
    pub metadata: bool,

    /// Use second pane as preview ? default to false
    #[arg(short = 'P', long, default_value_t = false)]
    pub preview: bool,

    /// Display all files (hidden)
    #[arg(short, long, default_value_t = false)]
    pub all: bool,
}
