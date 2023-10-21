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

    /// Dual pane ?
    #[arg(short = 'D', long)]
    pub dual: Option<bool>,

    /// Display files metadata ?
    #[arg(short = 'S', long)]
    pub simple: Option<bool>,

    /// Use second pane as preview ? default to false
    #[arg(short = 'P', long, default_value_t = false)]
    pub preview: bool,

    /// Display all files (hidden)
    #[arg(short = 'A', long, default_value_t = false)]
    pub all: bool,
}
