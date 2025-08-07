use std::{
    io::Write,
    os::unix::net::UnixStream,
    process::{Command, Stdio},
};

use anyhow::{bail, Result};
use clap::Parser;

use crate::io::Args;

/// Find a way to open the file in neovim.
/// Either, the process is ran from neovim itself, surelly from the plugin [fm-picker.nvim](https://github.com/qkzk/fm-picker.nvim)
/// or it's ran externally in another terminal window.
/// In the first case, an output socket should be provided through command line arguments and we use it.
/// The plugin listen on it and open the file in a new buffer.
/// In the second case, we send a a remote command to neovim.
/// Use `nvim --server $server_address --remote $filepath` to open the file in the neovim session.
///
/// I tried my best to avoid writing a plugin just for that but couldn't find another way since there's many ways to
/// open a terminal in neovim and they don't react to the same keybinds.
/// The problem I faced was to avoid opening the file _in the same window as the terminal_ since it may be floating...
pub fn nvim_open(server_address: &str, filepath: &std::path::Path) -> Result<()> {
    let args = Args::parse();
    if let Some(output_socket) = args.output_socket {
        nvim_inform_ipc(&output_socket, NvimIPCAction::OPEN(&filepath))
    } else {
        nvim_remote_send_open(server_address, filepath)
    }
}

fn nvim_remote_send_open(server_address: &str, filepath: &std::path::Path) -> Result<()> {
    if !std::path::Path::new(server_address).exists() {
        bail!("Neovim server {server_address} doesn't exists.");
    }
    let args = [
        "--server",
        server_address,
        "--remote",
        &filepath.to_string_lossy(),
    ];
    let output = Command::new("nvim")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.stdout.is_empty() || !output.stderr.is_empty() {
        crate::log_info!(
            "nvim {args:?}\nstdout: {stdout}\nstderr: {stderr}",
            stdout = String::from_utf8_lossy(&output.stdout),
            stderr = String::from_utf8_lossy(&output.stderr),
        );
    }

    Ok(())
}

#[non_exhaustive]
pub enum NvimIPCAction<'a, P>
where
    P: AsRef<std::path::Path>,
{
    OPEN(&'a P),
    DELETE(&'a P),
}

pub fn nvim_inform_ipc<P>(output_socket: &str, action: NvimIPCAction<P>) -> Result<()>
where
    P: AsRef<std::path::Path>,
{
    crate::log_info!("Using argument socket file {output_socket}");
    let mut stream = UnixStream::connect(output_socket)?;

    match action {
        NvimIPCAction::OPEN(filepath) => {
            writeln!(
                stream,
                "OPEN {filepath}",
                filepath = filepath.as_ref().display()
            )?;
            crate::log_info!(
                "Wrote to socket {output_socket}: OPEN {filepath}",
                filepath = filepath.as_ref().display()
            );
        }
        NvimIPCAction::DELETE(filepath) => {
            writeln!(
                stream,
                "DELETE {filepath}",
                filepath = filepath.as_ref().display()
            )?;
            crate::log_info!(
                "Wrote to socket {output_socket}: DELETE {filepath}",
                filepath = filepath.as_ref().display()
            );
        }
    }
    Ok(())
}
