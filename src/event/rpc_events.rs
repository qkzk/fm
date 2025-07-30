use std::{
    io::Read,
    os::unix::net::{UnixListener, UnixStream},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[non_exhaustive]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RpcEvent {
    Pick(String),
}

fn pid_socket_filepath() -> String {
    format!("/tmp/fm-socket-{pid}.sock", pid = std::process::id())
}

/// Creates the randomly named stream used by the application
/// Will be located in /tmp/socket_fm-abcdefg
/// Returns the pair "file_path, stream"
/// Read timeout is set to 1_000_000 ns = 0.001 s
pub fn create_stream() -> Result<(String, UnixListener)> {
    let file_path = pid_socket_filepath();
    let stream = UnixListener::bind(&file_path)?;
    crate::log_info!("created socket at {file_path}");
    Ok((file_path, stream))
}

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
