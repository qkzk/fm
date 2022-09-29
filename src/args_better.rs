#![allow(unused)]

use clap::Parser;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
pub struct Args {
    /// The path to the file to read
    pub path: std::path::PathBuf,
    /// hidden
    pub hidden: bool,
    pub help: bool,
    pub server: Option<String>,
}

fn main() {
    let args = Args::parse();
}
