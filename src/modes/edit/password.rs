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
    CRYPTSETUP(PasswordKind),
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
        self.sudo = Some(password);
    }

    /// Set the encrypted device passphrase
    pub fn set_cryptsetup(&mut self, passphrase: Password) {
        self.cryptsetup = Some(passphrase);
    }

    /// Reads the cryptsetup password
    #[must_use]
    pub const fn cryptsetup(&self) -> &Option<Password> {
        &self.cryptsetup
    }

    /// Reads the sudo password
    #[must_use]
    pub const fn sudo(&self) -> &Option<Password> {
        &self.sudo
    }

    /// True if the sudo password was set
    #[must_use]
    pub const fn has_sudo(&self) -> bool {
        self.sudo.is_some()
    }

    /// True if the encrypted device passphrase was set
    #[must_use]
    pub const fn has_cryptsetup(&self) -> bool {
        self.cryptsetup.is_some()
    }

    /// Reset every known password, dropping them.
    /// It should be called ASAP.
    pub fn reset(&mut self) {
        self.sudo = None;
        self.cryptsetup = None;
    }
}
