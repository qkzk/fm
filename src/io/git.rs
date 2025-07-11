// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// Copied and modified from https://github.com/9ary/gitprompt-rs/blob/master/src/main.rs
// Couldn't use without forking and I'm lazy.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::common::{is_in_path, set_current_dir};
use crate::io::execute_and_output_no_log;

#[derive(Default)]
struct GitStatus {
    branch: Option<String>,
    ahead: i64,
    behind: i64,

    staged: i64,
    modified: i64,
    deleted: i64,
    unmerged: i64,
    untracked: i64,
}

impl GitStatus {
    fn parse_porcelain2(porcerlain2_output: String) -> Option<GitStatus> {
        let mut status = GitStatus::default();
        // Simple parser for the porcelain v2 format
        for entry in porcerlain2_output.split('\0') {
            let mut entry = entry.split(' ');
            match entry.next() {
                // Header lines
                Some("#") => match entry.next()? {
                    "branch.head" => {
                        let head = entry.next()?;
                        if head != "(detached)" {
                            status.branch = Some(String::from(head));
                        }
                    }
                    "branch.ab" => {
                        let a = entry.next()?;
                        let b = entry.next()?;
                        status.ahead = a.parse::<i64>().ok()?.abs();
                        status.behind = b.parse::<i64>().ok()?.abs();
                    }
                    _ => {}
                },
                // File entries
                Some("1") | Some("2") => {
                    let mut xy = entry.next()?.chars();
                    let x = xy.next()?;
                    let y = xy.next()?;
                    if x != '.' {
                        status.staged += 1;
                    }
                    match y {
                        'M' => status.modified += 1,
                        'D' => status.deleted += 1,
                        _ => {}
                    }
                }
                Some("u") => status.unmerged += 1,
                Some("?") => status.untracked += 1,
                _ => {}
            }
        }
        Some(status)
    }

    fn is_modified(&self) -> bool {
        self.untracked + self.modified + self.deleted + self.unmerged + self.staged > 0
    }

    fn format_git_string(&self) -> Result<String> {
        let mut git_string = String::new();

        git_string.push('(');

        if let Some(branch) = &self.branch {
            git_string.push_str(branch);
        } else {
            // Detached head
            git_string.push_str(":HEAD");
        }

        // Divergence with remote branch
        if self.ahead != 0 {
            write!(git_string, "↑{}", self.ahead)?;
        }
        if self.behind != 0 {
            write!(git_string, "↓{}", self.behind)?;
        }

        if self.is_modified() {
            git_string.push('|');

            if self.untracked != 0 {
                write!(git_string, "+{}", self.untracked)?;
            }
            if self.modified != 0 {
                write!(git_string, "~{}", self.modified)?;
            }
            if self.deleted != 0 {
                write!(git_string, "-{}", self.deleted)?;
            }
            if self.unmerged != 0 {
                write!(git_string, "x{}", self.unmerged)?;
            }
            if self.staged != 0 {
                write!(git_string, "•{}", self.staged)?;
            }
        }

        git_string.push(')');

        Ok(git_string)
    }
}

fn porcelain2() -> Result<std::process::Output> {
    execute_and_output_no_log(
        "git",
        [
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=all",
        ],
    )
}

/// Returns a string representation of the git status of this path.
/// Will return an empty string if we're not in a git repository.
pub fn git(path: &Path) -> Result<String> {
    if !is_in_path("git") {
        return Ok("".to_owned());
    }
    if set_current_dir(path).is_err() {
        // The path may not exist. It should never happen.
        return Ok("".to_owned());
    }
    let output = porcelain2()?;
    if !output.status.success() {
        // We're most likely not in a Git repo
        return Ok("".to_owned());
    }
    let porcerlain_output = String::from_utf8(output.stdout)?;

    GitStatus::parse_porcelain2(porcerlain_output)
        .context("Error while parsing Git output")?
        .format_git_string()
}

/// Returns the git root.
/// Returns an error outside of a git repository.
pub fn git_root() -> Result<String> {
    let output = execute_and_output_no_log("git", ["rev-parse", "--show-toplevel"])?;
    if !output.status.success() {
        // We're most likely not in a Git repo
        return Err(anyhow!("git root: git command returned an error"));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
