use std::env;
use std::fmt;
use std::io::{stdout, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use nucleo::Injector;
use tokio::{
    io::AsyncBufReadExt, io::BufReader as TokioBufReader, process::Command as TokioCommand,
};

use crate::common::{current_username, is_in_path, GREP_EXECUTABLE, RG_EXECUTABLE, SETSID};
use crate::modes::PasswordHolder;
use crate::{log_info, log_line};

/// Execute a command with options in a fork with setsid.
/// If the `SETSID` application isn't there, call the program directly.
/// but the program may be closed if the parent (fm) is stopped.
/// Returns an handle to the child process.
///
/// # Errors
///
/// May fail if the command can't be spawned.
pub fn execute<S, P>(exe: S, args: &[P]) -> Result<std::process::Child>
where
    S: AsRef<std::ffi::OsStr> + fmt::Debug,
    P: AsRef<std::ffi::OsStr> + fmt::Debug,
{
    log_info!("execute. executable: {exe:?}, arguments: {args:?}");
    log_line!("Execute: {exe:?}, arguments: {args:?}");
    if is_in_path(SETSID) {
        Ok(Command::new(SETSID)
            .arg(exe)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?)
    } else {
        Ok(Command::new(exe)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?)
    }
}

/// Execute a command with options in a fork.
/// Returns an handle to the child process.
/// Branch stdin, stderr and stdout to /dev/null
pub fn execute_without_output<S: AsRef<std::ffi::OsStr> + fmt::Debug>(
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

pub fn execute_without_output_with_path<S, P>(
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
    if is_in_path(SETSID) {
        Ok(Command::new(SETSID)
            .arg(exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .current_dir(path)
            .args(params)
            .spawn()?)
    } else {
        Ok(Command::new(exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .current_dir(path)
            .args(params)
            .spawn()?)
    }
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
    let output = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;
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
    log_info!("execute_and_capture_output_with_path. executable: {exe:?}, arguments: {args:?}",);
    let output = Command::new(exe)
        .args(args)
        .current_dir(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        log_info!("{err}", err = String::from_utf8(output.stderr)?);
        Err(anyhow!(
            "execute_and_capture_output: command didn't finish properly",
        ))
    }
}

/// Execute a command with options in a fork.
/// Wait for termination and return either `Ok(stdout)`.
/// Branch stdin and stderr to /dev/null
pub fn execute_and_capture_output_without_check<S>(exe: S, args: &[&str]) -> Result<String>
where
    S: AsRef<std::ffi::OsStr> + fmt::Debug,
{
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
    Ok(Command::new(exe).args(args).stdin(Stdio::null()).output()?)
}

pub fn execute_with_ansi_colors(args: &[String]) -> Result<std::process::Output> {
    log_info!("execute. {args:?}");
    log_line!("Executed {args:?}");
    Ok(Command::new(&args[0])
        .args(&args[1..])
        .env("CLICOLOR_FORCE", "1")
        .env("COLORTERM", "ansi")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?)
}

pub fn execute_custom(exec_command: String, files: &[std::path::PathBuf]) -> Result<bool> {
    let mut args: Vec<&str> = exec_command.split(' ').collect();
    let command = args.remove(0);
    if !Path::new(command).exists() && !is_in_path(command) {
        log_info!("{command} can't be found - args {exec_command:?}");
        return Ok(false);
    }
    for file in files {
        args.push(file.to_str().context("Couldn't parse filepath to str")?);
    }
    execute(command, &args)?;
    Ok(true)
}

/// Spawn a sudo command with stdin, stdout and stderr piped.
/// sudo is run with -S argument to read the passworo from stdin
/// Args are sent.
/// CWD is set to `path`.
/// No password is set yet.
/// A password should be sent with `inject_password`.
fn new_sudo_command_awaiting_password<S, P>(args: &[S], path: P) -> Result<std::process::Child>
where
    S: AsRef<std::ffi::OsStr> + std::fmt::Debug,
    P: AsRef<std::path::Path> + std::fmt::Debug,
{
    Ok(Command::new("sudo")
        .arg("-S")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(path)
        .spawn()?)
}

/// Send password to a sudo command through its stdin.
fn inject_password(password: &str, child: &mut std::process::Child) -> Result<()> {
    let child_stdin = child
        .stdin
        .as_mut()
        .context("inject_password: couldn't open child stdin")?;
    child_stdin.write_all(format!("{password}\n").as_bytes())?;
    Ok(())
}

/// run a sudo command requiring a password (generally to establish the password.)
/// Since I can't send 2 passwords at a time, it will only work with the sudo password
/// It requires a path to establish CWD.
pub fn execute_sudo_command_with_password<S, P>(
    args: &[S],
    password: &str,
    path: P,
) -> Result<(bool, String, String)>
where
    S: AsRef<std::ffi::OsStr> + std::fmt::Debug,
    P: AsRef<std::path::Path> + std::fmt::Debug,
{
    log_info!("sudo_with_password {args:?} CWD {path:?}");
    log_line!("running sudo command with password. args: {args:?}, CWD: {path:?}");
    let mut child = new_sudo_command_awaiting_password(args, path)?;
    inject_password(password, &mut child)?;
    let output = child.wait_with_output()?;
    Ok((
        output.status.success(),
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}

/// Spawn a sudo command which shouldn't require a password.
/// The command is executed immediatly and we return an handle to it.
fn new_sudo_command_passwordless<S>(args: &[S]) -> Result<std::process::Child>
where
    S: AsRef<std::ffi::OsStr> + std::fmt::Debug,
{
    Ok(Command::new("sudo")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?)
}

/// Runs a passwordless sudo command.
/// Returns stdout & stderr
pub fn execute_sudo_command<S>(args: &[S]) -> Result<(bool, String, String)>
where
    S: AsRef<std::ffi::OsStr> + std::fmt::Debug,
{
    log_info!("running sudo {:?}", args);
    log_line!("running sudo command. {args:?}");
    let child = new_sudo_command_passwordless(args)?;
    let output = child.wait_with_output()?;
    Ok((
        output.status.success(),
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}

/// Runs `sudo -k` removing sudo privileges of current running instance.
pub fn drop_sudo_privileges() -> Result<()> {
    Command::new("sudo")
        .arg("-k")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

/// Reset the sudo faillock to avoid being blocked from running sudo commands.
/// Runs `faillock --user $USERNAME --reset`
pub fn reset_sudo_faillock() -> Result<()> {
    Command::new("faillock")
        .arg("--user")
        .arg(current_username()?)
        .arg("--reset")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

/// Execute `sudo -S ls -l /root`, passing the password into `stdin`.
/// It sets a sudo session which will be reset later.
pub fn set_sudo_session(password: &PasswordHolder) -> Result<bool> {
    let root_path = std::path::Path::new("/");
    // sudo
    let (success, _, _) = execute_sudo_command_with_password(
        &["ls", "/root"],
        password
            .sudo()
            .as_ref()
            .context("sudo password isn't set")?,
        root_path,
    )?;
    Ok(success)
}

#[tokio::main]
pub async fn inject(mut command: TokioCommand, injector: Injector<String>) {
    let Ok(mut cmd) = command
        .stdout(Stdio::piped()) // Can do the same for stderr
        .spawn()
    else {
        log_info!("Cannot spawn command");
        return;
    };
    let Some(stdout) = cmd.stdout.take() else {
        log_info!("no stdout");
        return;
    };
    let mut lines = TokioBufReader::new(stdout).lines();
    while let Ok(opt_line) = lines.next_line().await {
        let Some(line) = opt_line else {
            break;
        };
        injector.push(line.clone(), |line, cols| {
            cols[0] = line.as_str().into();
        });
    }
}

pub fn build_tokio_greper() -> Option<TokioCommand> {
    let shell_command = if is_in_path(RG_EXECUTABLE) {
        RG_EXECUTABLE
    } else if is_in_path(GREP_EXECUTABLE) {
        GREP_EXECUTABLE
    } else {
        return None;
    };
    let mut args: Vec<_> = shell_command.split_whitespace().collect();
    if args.is_empty() {
        return None;
    }
    let grep = args.remove(0);
    let mut tokio_greper = TokioCommand::new(grep);
    tokio_greper.args(&args);
    Some(tokio_greper)
}

/// Open a new shell in current window.
/// Disable raw mode, clear the screen, start a new shell ($SHELL, default to bash).
/// Wait...
/// Once the shell exits,
/// Clear the screen and renable raw mode.
///
/// It's the responsability of the caller to ensure displayer doesn't try to override the display.
pub fn open_shell_in_window() -> Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), DisableMouseCapture, Clear(ClearType::All))?;

    let shell = env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
    let shell_status = Command::new(&shell).status()?;

    if !shell_status.success() {
        log_info!(
            "Shell {shell} exited with non-zero status: {:?}",
            shell_status
        );
    }

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnableMouseCapture, Clear(ClearType::All))?;
    Ok(())
}

pub fn open_command_in_window(args: &[&str]) -> Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), DisableMouseCapture, Clear(ClearType::All))?;

    let shell = env::var("SHELL").unwrap_or_else(|_| "bash".to_string());
    let mut shell_command = Command::new(&shell);
    shell_command.arg("-c").args(args);
    log_info!("open_file_in_window {shell_command:?}");
    let shell_status = shell_command.status()?;

    if !shell_status.success() {
        log_info!("Shell exited with non-zero status: {:?}", shell_status);
    }

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnableMouseCapture, Clear(ClearType::All))?;
    Ok(())
}
