use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};

use crate::{log_info, log_line};

/// Execute a command with options in a fork.
/// Returns an handle to the child process.
pub fn execute_in_child<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<std::process::Child> {
    log_info!("execute_in_child. executable: {exe:?}, arguments: {args:?}");
    log_line!("Execute: {exe:?}, arguments: {args:?}");
    Ok(Command::new(exe).args(args).spawn()?)
}

/// Execute a command with options in a fork.
/// Returns an handle to the child process.
/// Branch stdin, stderr and stdout to /dev/null
pub fn execute_in_child_without_output<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<std::process::Child> {
    log_info!("execute_in_child_without_output. executable: {exe:?}, arguments: {args:?}",);
    Ok(Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?)
}

pub fn execute_in_child_without_output_with_path<S, P>(
    exe: S,
    path: P,
    args: Option<&[&str]>,
) -> Result<std::process::Child>
where
    S: AsRef<std::ffi::OsStr> + fmt::Debug,
    P: AsRef<Path>,
{
    log_info!(
        "execute_in_child_without_output_with_path. executable: {exe:?}, arguments: {args:?}"
    );
    let params = args.unwrap_or(&[]);
    Ok(Command::new(exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(path)
        .args(params)
        .spawn()?)
}
/// Execute a command with options in a fork.
/// Wait for termination and return either :
/// `Ok(stdout)` if the status code is 0
/// an Error otherwise
/// Branch stdin and stderr to /dev/null
pub fn execute_and_capture_output<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<String> {
    log_info!("execute_and_capture_output. executable: {exe:?}, arguments: {args:?}",);
    let child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(anyhow!(
            "execute_and_capture_output: command didn't finish properly",
        ))
    }
}

/// Execute a command with options in a fork.
/// Wait for termination and return either :
/// `Ok(stdout)` if the status code is 0
/// an Error otherwise
/// Branch stdin and stderr to /dev/null
pub fn execute_and_capture_output_with_path<
    S: AsRef<std::ffi::OsStr> + fmt::Debug,
    P: AsRef<Path>,
>(
    exe: S,
    path: P,
    args: &[&str],
) -> Result<String> {
    log_info!("execute_and_capture_output. executable: {exe:?}, arguments: {args:?}",);
    let child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(path)
        .spawn()?;
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(anyhow!(
            "execute_and_capture_output: command didn't finish properly",
        ))
    }
}

/// Execute a command with options in a fork.
/// Wait for termination and return either `Ok(stdout)`.
/// Branch stdin and stderr to /dev/null
pub fn execute_and_capture_output_without_check<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
    exe: S,
    args: &[&str],
) -> Result<String> {
    log_info!("execute_and_capture_output_without_check. executable: {exe:?}, arguments: {args:?}",);
    let child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let output = child.wait_with_output()?;
    Ok(String::from_utf8(output.stdout)?)
}

pub fn execute_and_output<S, I>(exe: S, args: I) -> Result<std::process::Output>
where
    S: AsRef<std::ffi::OsStr> + fmt::Debug,
    I: IntoIterator<Item = S> + fmt::Debug,
{
    log_info!("execute_and_output. executable: {exe:?}, arguments: {args:?}",);
    Ok(Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?)
}

pub fn execute_and_output_no_log<S, I>(exe: S, args: I) -> Result<std::process::Output>
where
    S: AsRef<std::ffi::OsStr> + fmt::Debug,
    I: IntoIterator<Item = S> + fmt::Debug,
{
    Ok(Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?)
}
