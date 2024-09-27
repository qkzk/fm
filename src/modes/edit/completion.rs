use std::fmt;
use std::fs::{self, ReadDir};

use anyhow::Result;
use strum::IntoEnumIterator;

use crate::common::{is_in_path, tilde, ZOXIDE};
use crate::event::ActionMap;
use crate::io::execute_and_capture_output_with_path;
use crate::modes::Leave;
use crate::{impl_content, impl_selectable};

/// Different kind of completions
#[derive(Clone, Default, Copy)]
pub enum InputCompleted {
    #[default]
    /// Complete a directory path in filesystem
    Cd,
    /// Complete a filename from current directory
    Search,
    /// Complete an executable name from $PATH
    Exec,
    /// Complete with an existing action
    Action,
}

impl fmt::Display for InputCompleted {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            #[rustfmt::skip]
            Self::Exec      => write!(f, "Open with: "),
            #[rustfmt::skip]
            Self::Cd        => write!(f, "Cd:        "),
            #[rustfmt::skip]
            Self::Search    => write!(f, "Search:    "),
            #[rustfmt::skip]
            Self::Action    => write!(f, "Action:    "),
        }
    }
}

impl InputCompleted {
    pub fn cursor_offset(&self) -> usize {
        self.to_string().len() + 2
    }
}

impl Leave for InputCompleted {
    fn must_refresh(&self) -> bool {
        true
    }

    fn must_reset_mode(&self) -> bool {
        !matches!(self, Self::Action)
    }
}

/// Holds a `Vec<String>` of possible completions and an `usize` index
/// showing where the user is in the vec.
#[derive(Clone, Default)]
pub struct Completion {
    /// Possible completions
    pub content: Vec<String>,
    /// Which completion is selected by the user
    pub index: usize,
}

impl Completion {
    /// Is there any completion option ?
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the currently selected proposition.
    /// Returns an empty string if `proposals` is empty.
    pub fn current_proposition(&self) -> &str {
        if self.content.is_empty() {
            return "";
        }
        &self.content[self.index]
    }

    /// Updates the proposition with a new `Vec`.
    /// Reset the index to 0.
    fn update(&mut self, proposals: Vec<String>) {
        self.index = 0;
        self.content = proposals;
        self.content.dedup()
    }

    fn extend(&mut self, proposals: &[String]) {
        self.index = 0;
        self.content.extend_from_slice(proposals);
        self.content.dedup()
    }

    /// Empty the proposals `Vec`.
    /// Reset the index.
    pub fn reset(&mut self) {
        self.index = 0;
        self.content.clear();
    }

    /// Cd completion.
    /// Looks for the valid path completing what the user typed.
    pub fn cd(&mut self, input_string: &str, current_path: &str) -> Result<()> {
        self.cd_update_from_input(input_string, current_path);
        let (parent, last_name) = split_input_string(input_string);
        if last_name.is_empty() {
            return Ok(());
        }
        self.extend_absolute_paths(&parent, &last_name);
        self.extend_relative_paths(current_path, &last_name);
        Ok(())
    }

    fn cd_update_from_input(&mut self, input_string: &str, current_path: &str) {
        self.content = vec![];
        self.cd_update_from_zoxide(input_string, current_path);
        if let Some(expanded_input) = self.expand_input(input_string) {
            self.content.push(expanded_input);
        }
        if let Some(cannonicalized_input) = self.canonicalize_input(input_string, current_path) {
            self.content.push(cannonicalized_input);
        }
    }

    fn cd_update_from_zoxide(&mut self, input_string: &str, current_path: &str) {
        if !is_in_path(ZOXIDE) {
            return;
        }
        let mut args = vec!["query"];
        args.extend(input_string.split(' '));
        let Ok(zoxide_output) = execute_and_capture_output_with_path(ZOXIDE, current_path, &args)
        else {
            return;
        };
        if !zoxide_output.is_empty() {
            self.content.push(zoxide_output.trim().to_string());
        }
    }

    fn expand_input(&mut self, input_string: &str) -> Option<String> {
        let expanded_input = tilde(input_string).into_owned();
        if std::path::PathBuf::from(&expanded_input).exists() {
            Some(expanded_input)
        } else {
            None
        }
    }

    fn canonicalize_input(&mut self, input_string: &str, current_path: &str) -> Option<String> {
        let mut path = fs::canonicalize(current_path).unwrap_or_default();
        path.push(input_string);
        let path = fs::canonicalize(path).unwrap_or_default();
        if path.exists() {
            Some(path.to_str().unwrap_or_default().to_owned())
        } else {
            None
        }
    }

    fn extend_absolute_paths(&mut self, parent: &str, last_name: &str) {
        let Ok(path) = std::fs::canonicalize(parent) else {
            return;
        };
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        self.extend(&Self::entries_matching_filename(entries, last_name))
    }

    fn extend_relative_paths(&mut self, current_path: &str, last_name: &str) {
        if let Ok(entries) = fs::read_dir(current_path) {
            self.extend(&Self::entries_matching_filename(entries, last_name))
        }
    }

    fn entries_matching_filename(entries: ReadDir, last_name: &str) -> Vec<String> {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_ok())
            .filter(|e| e.file_type().unwrap().is_dir() && filename_startswith(e, last_name))
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect()
    }

    /// Looks for programs in $PATH completing the one typed by the user.
    pub fn exec(&mut self, input_string: &str) -> Result<()> {
        let mut proposals: Vec<String> = vec![];
        if let Some(paths) = std::env::var_os("PATH") {
            for path in std::env::split_paths(&paths).filter(|path| path.exists()) {
                proposals.extend(Self::find_completion_in_path(path, input_string)?);
            }
        }
        self.update(proposals);
        Ok(())
    }

    /// Looks for fm actions completing the one typed by the user.
    pub fn command(&mut self, input_string: &str) -> Result<()> {
        let proposals = ActionMap::iter()
            .filter(|command| {
                command
                    .to_string()
                    .to_lowercase()
                    .contains(&input_string.to_lowercase())
            })
            .map(|command| command.to_string())
            .collect();
        self.update(proposals);
        Ok(())
    }

    fn find_completion_in_path(
        path: std::path::PathBuf,
        input_string: &str,
    ) -> Result<Vec<String>> {
        Ok(fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| file_match_input(e, input_string))
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect())
    }

    /// Looks for file within current folder completing what the user typed.
    pub fn search(&mut self, files: Vec<String>) {
        self.update(files);
    }

    /// Complete the input string with current_proposition if possible.
    /// Returns the optional last chars of the current_proposition.
    /// If the current_proposition doesn't start with input_string, it returns None.
    pub fn complete_input_string(&self, input_string: &str) -> Option<&str> {
        self.current_proposition().strip_prefix(input_string)
    }

    /// Reverse the received effect if the index match the selected index.
    pub fn attr(&self, index: usize, attr: &tuikit::attr::Attr) -> tuikit::attr::Attr {
        let mut attr = *attr;
        if index == self.index {
            attr.effect |= tuikit::attr::Effect::REVERSE;
        }
        attr
    }
}

fn file_match_input(dir_entry: &std::fs::DirEntry, input_string: &str) -> bool {
    let Ok(file_type) = dir_entry.file_type() else {
        return false;
    };
    (file_type.is_file() || file_type.is_symlink()) && filename_startswith(dir_entry, input_string)
}

/// true if the filename starts with a pattern
fn filename_startswith(entry: &std::fs::DirEntry, pattern: &str) -> bool {
    entry
        .file_name()
        .to_string_lossy()
        .as_ref()
        .starts_with(pattern)
}

fn split_input_string(input_string: &str) -> (String, String) {
    let steps = input_string.split('/');
    let mut vec_steps: Vec<&str> = steps.collect();
    let last_name = vec_steps.pop().unwrap_or("").to_owned();
    let parent = create_parent(vec_steps);
    (parent, last_name)
}

fn create_parent(vec_steps: Vec<&str>) -> String {
    let mut parent = if vec_steps.is_empty() || vec_steps.len() == 1 && vec_steps[0] != "~" {
        "/".to_owned()
    } else {
        "".to_owned()
    };
    parent.push_str(&vec_steps.join("/"));
    tilde(&parent).to_string()
}

impl_selectable!(Completion);
impl_content!(String, Completion);

use crate::io::DrawMenu;

impl DrawMenu<InputCompleted, String> for Completion {}
