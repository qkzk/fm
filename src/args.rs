use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
/// FM : dired like file manager{n}
pub struct Args {
    /// Starting path
    #[arg(short, long, default_value_t = String::from("."))]
    pub path: String,

    /// Display all files
    #[arg(short, long, default_value_t = false)]
    pub all: bool,

    /// Nvim server
    #[arg(short, long, default_value_t = String::from(""))]
    pub server: String,
}
