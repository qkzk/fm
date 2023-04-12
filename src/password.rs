use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use log::info;

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
pub fn sudo_password(args: &[String], password: &str) -> Result<(bool, String, String)> {
    info!("sudo {args:?}, {password}");
    let mut child = Command::new("sudo")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
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

/// Run a passwordless sudo command.
/// Returns stdout & stderr
pub fn sudo(args: &[String]) -> Result<(bool, String, String)> {
    info!("sudo {:?}", args);
    let child = Command::new("sudo")
        .args(args)
        .stdin(Stdio::piped())
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
