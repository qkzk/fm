use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::common::current_username;
use crate::log_info;
use crate::log_line;

/// Different kind of password
#[derive(Debug, Clone, Copy)]
pub enum PasswordKind {
    SUDO,
    CRYPTSETUP,
}

impl std::fmt::Display for PasswordKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let asker = match self {
            Self::SUDO => "sudo   ",
            Self::CRYPTSETUP => "device ",
        };
        write!(f, "{asker}")
    }
}

/// What will this password be used for ?
/// ATM only 3 usages are supported:
/// * mounting an ISO file,
/// * opening an mounting an encrypted device.
/// * running a sudo command
#[derive(Debug, Clone, Copy)]
pub enum PasswordUsage {
    ISO,
    CRYPTSETUP,
    SUDOCOMMAND,
}

type Password = String;

/// Holds passwords allowing to mount or unmount an encrypted drive.
#[derive(Default, Clone, Debug)]
pub struct PasswordHolder {
    sudo: Option<Password>,
    cryptsetup: Option<Password>,
}

impl PasswordHolder {
    /// Set the sudo password.
    pub fn set_sudo(&mut self, password: Password) {
        self.sudo = Some(password)
    }

    /// Set the encrypted device passphrase
    pub fn set_cryptsetup(&mut self, passphrase: Password) {
        self.cryptsetup = Some(passphrase)
    }

    /// Reads the cryptsetup password
    pub fn cryptsetup(&self) -> &Option<Password> {
        &self.cryptsetup
    }

    /// Reads the sudo password
    pub fn sudo(&self) -> &Option<Password> {
        &self.sudo
    }

    /// True if the sudo password was set
    pub fn has_sudo(&self) -> bool {
        self.sudo.is_some()
    }

    /// True if the encrypted device passphrase was set
    pub fn has_cryptsetup(&self) -> bool {
        self.cryptsetup.is_some()
    }

    /// Reset every known password, dropping them.
    /// It should be called ASAP.
    pub fn reset(&mut self) {
        self.sudo = None;
        self.cryptsetup = None;
    }
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
        .context("run_privileged_command: couldn't open child stdin")?;
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
        &password
            .sudo()
            .as_ref()
            .context("sudo password isn't set")?,
        root_path,
    )?;
    Ok(success)
}