use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
/// FM : dired like file manager{n}
pub struct Args {
    /// Starting path
    #[arg(short, long, default_value_t = String::from("."))]
    pub path: String,

    /// Nvim server
    #[arg(short, long, default_value_t = String::from(""))]
    pub server: String,
}
