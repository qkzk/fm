use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
/// FM : dired / ranger like file manager{n} {n}Config files: ~/.config/fm/{n}Documentation: <https://github.com/qkzk/fm>{n}
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

    /// fm is started inside neovim terminal emulator
    #[arg(long, default_value_t = false)]
    pub neovim: bool,

    /// UNIX Socket file by fm to receive messages
    #[arg(long)]
    pub input_socket: Option<String>,

    /// UNIX Socket file used by fm to send messages
    #[arg(long)]
    pub output_socket: Option<String>,

    /// Disable images previewing
    #[arg(long, default_value_t = false)]
    pub disable_images: bool,
}
