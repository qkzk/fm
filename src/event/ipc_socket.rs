use std::{
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
};

use anyhow::{bail, Result};
use clap::Parser;

use crate::io::Args;

/// filepath of the socked used
/// If the user provided a filepath (will be the case if you use neovim
/// companion plugin [fm-picker.nvim}(https://github.com/qkzk/fm-picker.nvim))
/// then we use it.
/// Otherwise, it's `/tmp/fm-socket-{pid}.sock` where `pid` is the process
/// identifier of the current process.
pub fn build_input_socket_filepath() -> String {
    let args = Args::parse();
    if let Some(socket_adress) = args.input_socket {
        crate::log_info!("Using socket provided in args : #{socket_adress}#");
        socket_adress
    } else {
        format!("/tmp/fm-socket-{pid}.sock", pid = std::process::id())
    }
}

/// Creates UNIX socket stream used by the application
/// If the user provided an input socket from args, it will use it. Otherwise, it will use "/tmp/fm-socket-{pid}.sock"
/// where pid is the process identifier of the application itself.
/// Read timeout is set to 1_000_000 ns = 0.001 s
/// Returns the pair "file_path, stream"
pub fn create_stream() -> Result<(String, UnixListener)> {
    let file_path = build_input_socket_filepath();
    let stream = match UnixListener::bind(&file_path) {
        Ok(stream) => stream,
        Err(err) => {
            crate::log_info!("Couldn't create stream. {file_path}");
            bail!(err)
        }
    };
    std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o600))?;
    crate::log_info!("created output socket at {file_path}");
    Ok((file_path, stream))
}

/// Read from an UNIX socket stream and return its output as a `String`.
pub fn read_from_stream(stream: &mut UnixStream) -> Option<String> {
    let mut buffer = String::new();
    stream.read_to_string(&mut buffer).ok()?;
    if !buffer.is_empty() {
        crate::log_info!("read from socket: ####{buffer}");
        Some(buffer)
    } else {
        None
    }
}

/// Writes a string to an UNIX socket.
///
/// # Errors
///
/// May fail if the unix socket is closed or if the user can't write to it.
pub fn write_to_stream(stream: &mut UnixStream, data: String) -> Result<()> {
    stream.write_all(data.as_bytes())?;
    Ok(())
}
