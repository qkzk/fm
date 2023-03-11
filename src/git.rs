// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// Copied from https://github.com/9ary/gitprompt-rs/blob/master/src/main.rs
// Couldn't use without forking and I'm lazy.

use anyhow::{anyhow, Context, Result};
use std::fmt::Write as _;
use std::path::Path;
use std::process;

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

fn parse_porcelain2(data: String) -> Option<GitStatus> {
    let mut status = GitStatus {
        branch: None,
        ahead: 0,
        behind: 0,

        staged: 0,
        modified: 0,
        deleted: 0,
        unmerged: 0,
        untracked: 0,
    };
    // Simple parser for the porcelain v2 format
    for entry in data.split('\0') {
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

/// Returns a string representation of the git status of this path.
/// Will return an empty string if we're not in a git repository.
pub fn git(path: &Path) -> Result<String> {
    if std::env::set_current_dir(path).is_err() {
        // The path may not exist. It should never happen.
        return Ok("".to_owned());
    }
    let output = process::Command::new("git")
        .args([
            "status",
            "--porcelain=v2",
            "-z",
            "--branch",
            "--untracked-files=all",
        ])
        .stdin(process::Stdio::null())
        .stderr(process::Stdio::null())
        .output()?;
    if !output.status.success() {
        // We're most likely not in a Git repo
        return Ok("".to_owned());
    }
    let status = String::from_utf8(output.stdout)
        .ok()
        .context("Invalid UTF-8 while decoding Git output")?;

    let status = parse_porcelain2(status).context("Error while parsing Git output")?;

    let mut git_string = String::new();

    git_string.push('(');

    if let Some(branch) = status.branch {
        git_string.push_str(&branch);
    } else {
        // Detached head
        git_string.push_str(":HEAD");
    }

    // Divergence with remote branch
    if status.ahead != 0 {
        write!(git_string, "↑{}", status.ahead)?;
    }
    if status.behind != 0 {
        write!(git_string, "↓{}", status.ahead)?;
    }

    if status.untracked + status.modified + status.deleted + status.unmerged + status.staged > 0 {
        git_string.push('|');
    }
    if status.untracked != 0 {
        write!(git_string, "+{}", status.untracked)?;
    }
    if status.modified != 0 {
        write!(git_string, "~{}", status.modified)?;
    }
    if status.deleted != 0 {
        write!(git_string, "-{}", status.deleted)?;
    }
    if status.unmerged != 0 {
        write!(git_string, "x{}", status.unmerged)?;
    }
    if status.staged != 0 {
        write!(git_string, "•{}", status.staged)?;
    }

    git_string.push(')');

    Ok(git_string)
}

/// Returns the git root.
/// Returns an error outside of a git repository.
pub fn git_root() -> Result<String> {
    let output = process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stdin(process::Stdio::null())
        .stderr(process::Stdio::null())
        .output()?;

    if !output.status.success() {
        // We're most likely not in a Git repo
        return Err(anyhow!("git root: git command returned an error"));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
