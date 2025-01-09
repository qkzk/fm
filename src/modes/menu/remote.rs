use std::process::Command;

use crate::common::{is_in_path, SSHFS_EXECUTABLE};
use crate::io::{command_with_path, execute_and_capture_output_with_path};
use crate::{log_info, log_line};

/// Used to remember the setting of this remote.
/// it's used to mount the remote in the current directory using sshfs.
pub struct Remote {
    pub username: String,
    pub hostname: String,
    pub remote_path: String,
    pub local_path: String,
    pub current_path: String,
    pub port: String,
}

impl Remote {
    /// Converts an input like "username hostname ~ 33" into
    /// a separated list of arguments for sshfs.
    /// Returns `None` if the input string can't be parsed correctly.
    /// Returns also `None` if "sshfs" isn't in the current `$PATH`.
    pub fn from_input(input: String, current_path: &str) -> Option<Self> {
        if !is_in_path(SSHFS_EXECUTABLE) {
            log_info!("{SSHFS_EXECUTABLE} isn't in path");
            return None;
        }

        Self::parse_remote_args(input, current_path)
    }

    fn parse_remote_args(input: String, current_path: &str) -> Option<Self> {
        let user_host_port_remotepath_localpath: Vec<&str> = input.trim().split(' ').collect();
        let number_of_args = user_host_port_remotepath_localpath.len();
        if number_of_args != 3 && number_of_args != 4 {
            log_info!(
                "Wrong number of parameters for {SSHFS_EXECUTABLE}, expected 3 or 4, got {number_of_args}",
            );
            return None;
        };

        let (username, host_port, remote_path) = (
            user_host_port_remotepath_localpath[0],
            user_host_port_remotepath_localpath[1],
            user_host_port_remotepath_localpath[2],
        );

        let host_port_splitted: Vec<&str> = host_port.trim().split(':').collect();
        let hostname = host_port_splitted[0];
        let port = if host_port_splitted.len() == 1 {
            "22"
        } else {
            host_port_splitted[1]
        };

        let local_path = if number_of_args == 3 {
            current_path
        } else {
            user_host_port_remotepath_localpath[3]
        };
        Some(Self {
            username: username.to_owned(),
            hostname: hostname.to_owned(),
            remote_path: remote_path.to_owned(),
            current_path: current_path.to_owned(),
            local_path: local_path.to_owned(),
            port: port.to_owned(),
        })
    }

    pub fn command(&self) -> Command {
        let first_arg = format!(
            "{username}@{hostname}:{remote_path}",
            username = self.username,
            hostname = self.hostname,
            remote_path = self.remote_path
        );
        command_with_path(
            SSHFS_EXECUTABLE,
            &self.current_path,
            &[&first_arg, &self.local_path, "-p", &self.port],
        )
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    pub fn mount(&self) {
        let first_arg = format!(
            "{username}@{hostname}:{remote_path}",
            username = self.username,
            hostname = self.hostname,
            remote_path = self.remote_path
        );
        let output = execute_and_capture_output_with_path(
            SSHFS_EXECUTABLE,
            &self.current_path,
            &[&first_arg, &self.local_path, "-p", &self.port],
        );
        log_info!("{SSHFS_EXECUTABLE} {first_arg} output {output:?}");
        log_line!("{SSHFS_EXECUTABLE} {first_arg} output {output:?}");
    }
}
