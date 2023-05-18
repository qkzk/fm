use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use log::info;

use crate::utils::current_username;

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
/// ATM only 2 usages are supported:
/// * mounting an ISO file,
/// * opening an mounting an encrypted device.
#[derive(Debug, Clone, Copy)]
pub enum PasswordUsage {
    ISO,
    CRYPTSETUP,
    SUDOCOMMAND,
}

/// Holds passwords allowing to mount or unmount an encrypted drive.
#[derive(Default, Clone, Debug)]
pub struct PasswordHolder {
    sudo: Option<String>,
    cryptsetup: Option<String>,
}

impl PasswordHolder {
    /// Set the sudo password.
    pub fn set_sudo(&mut self, password: String) {
        self.sudo = Some(password)
    }

    /// Set the encrypted device passphrase
    pub fn set_cryptsetup(&mut self, passphrase: String) {
        self.cryptsetup = Some(passphrase)
    }

    /// Reads the cryptsetup password
    pub fn cryptsetup(&self) -> Result<String> {
        self.cryptsetup
            .clone()
            .context("PasswordHolder: cryptsetup password isn't set")
    }

    /// Reads the sudo password
    pub fn sudo(&self) -> Result<String> {
        self.sudo
            .clone()
            .context("PasswordHolder: sudo password isn't set")
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
    info!("sudo_with_password {args:?} CWD {path:?}");
    info!(
        target: "special",
        "running sudo command with passwod. args: {args:?}, CWD: {path:?}"
    );
    let mut child = Command::new("sudo")
        .arg("-S")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(path)
        .spawn()?;

    let child_stdin = child
        .stdin
        .as_mut()
        .context("run_privileged_command: couldn't open child stdin")?;
    child_stdin.write_all(format!("{password}\n").as_bytes())?;

    let output = child.wait_with_output()?;
    Ok((
        output.status.success(),
        String::from_utf8(output.stdout)?,
        String::from_utf8(output.stderr)?,
    ))
}

/// Runs a passwordless sudo command.
/// Returns stdout & stderr
pub fn execute_sudo_command<S>(args: &[S]) -> Result<(bool, String, String)>
where
    S: AsRef<std::ffi::OsStr> + std::fmt::Debug,
{
    info!("running sudo {:?}", args);
    info!(target: "special", "running sudo command. {args:?}");
    let child = Command::new("sudo")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
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
