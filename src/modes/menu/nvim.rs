use std::process::{Command, Stdio};

use anyhow::Result;

/// Use `nvim --server $server_address --remote $filepath` to open the file in the neovim session.
pub fn nvim(server_address: &str, filepath: &std::path::Path) -> Result<()> {
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
