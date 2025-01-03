use crate::common::{is_in_path, SSHFS_EXECUTABLE};
use crate::io::execute_and_capture_output_with_path;
use crate::{log_info, log_line};

/// Used to remember the setting of this remote.
/// it's used to mount the remote in the current directory using sshfs.
pub struct Remote {
    username: String,
    hostname: String,
    remote_path: String,
    port: String,
}

impl Remote {
    /// Converts an input like "username hostname ~ 33" into
    /// a separated list of arguments for sshfs.
    /// Returns `None` if the input string can't be parsed correctly.
    /// Returns also `None` if "sshfs" isn't in the current `$PATH`.
    pub fn from_input(input: String) -> Option<Self> {
        if !is_in_path(SSHFS_EXECUTABLE) {
            log_info!("{SSHFS_EXECUTABLE} isn't in path");
            return None;
        }
        let user_hostname_path_port: Vec<&str> = input.trim().split(' ').collect();
        let (username, hostname, remote_path, port) =
            Self::parse_remote_args(user_hostname_path_port)?;
        Some(Self {
            username,
            hostname,
            remote_path,
            port,
        })
    }

    fn parse_remote_args(
        user_hostname_path_port: Vec<&str>,
    ) -> Option<(String, String, String, String)> {
        let number_of_args = user_hostname_path_port.len();
        if number_of_args != 3 && number_of_args != 4 {
            log_info!(
                "Wrong number of parameters for {SSHFS_EXECUTABLE}, expected 3 or 4, got {number_of_args}",
            );
            return None;
        };

        let (username, hostname, remote_path) = (
            user_hostname_path_port[0],
            user_hostname_path_port[1],
            user_hostname_path_port[2],
        );

        let port = if number_of_args == 3 {
            "22"
        } else {
            user_hostname_path_port[3]
        };
        Some((
            username.to_owned(),
            hostname.to_owned(),
            remote_path.to_owned(),
            port.to_owned(),
        ))
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    pub fn mount(&self, current_path: &str) {
        let first_arg = format!(
            "{username}@{hostname}:{remote_path}",
            username = self.username,
            hostname = self.hostname,
            remote_path = self.remote_path
        );
        let output = execute_and_capture_output_with_path(
            SSHFS_EXECUTABLE,
            current_path,
            &[&first_arg, current_path, "-p", &self.port],
        );
        log_info!("{SSHFS_EXECUTABLE} {first_arg} output {output:?}");
        log_line!("{SSHFS_EXECUTABLE} {first_arg} output {output:?}");
    }
}
