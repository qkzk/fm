use std::fs::{self, ReadDir};

use strum::IntoEnumIterator;

use crate::fileinfo::PathContent;
use crate::fm_error::FmResult;
use crate::mode::Mode;
use crate::tree::ColoredString;

/// Different kind of completions
#[derive(Clone, Default, Copy)]
pub enum InputCompleted {
    /// No completion needed
    #[default]
    Nothing,
    /// Complete a directory path in filesystem
    Goto,
    /// Complete a filename from current directory
    Search,
    /// Complete an executable name from $PATH
    Exec,
    /// Command
    Command,
}

/// Holds a `Vec<String>` of possible completions and an `usize` index
/// showing where the user is in the vec.
#[derive(Clone, Default)]
pub struct Completion {
    kind: InputCompleted,
    /// Possible completions
    pub proposals: Vec<String>,
    /// Which completion is selected by the user
    pub index: usize,
}

impl Completion {
    pub fn set_kind(&mut self, mode: &Mode) {
        if let Mode::InputCompleted(completion_kind) = mode {
            self.kind = *completion_kind
        } else {
            self.kind = InputCompleted::Nothing
        }
    }

    /// Is there any completion option ?
    pub fn is_empty(&self) -> bool {
        self.proposals.is_empty()
    }

    /// Move the index to next element, cycling to 0.
    /// Does nothing if the list is empty.
    pub fn next(&mut self) {
        if self.proposals.is_empty() {
            return;
        }
        self.index = (self.index + 1) % self.proposals.len()
    }

    /// Move the index to previous element, cycling to the last one.
    /// Does nothing if the list is empty.
    pub fn prev(&mut self) {
        if self.proposals.is_empty() {
            return;
        }
        if self.index > 0 {
            self.index -= 1
        } else {
            self.index = self.proposals.len() - 1
        }
    }

    /// Returns the currently selected proposition.
    /// Returns an empty string if `proposals` is empty.
    pub fn current_proposition(&self) -> &str {
        if self.proposals.is_empty() {
            return "";
        }
        &self.proposals[self.index]
    }

    /// Updates the proposition with a new `Vec`.
    /// Reset the index to 0.
    fn update(&mut self, proposals: Vec<String>) {
        self.index = 0;
        self.proposals = proposals;
    }

    fn extend(&mut self, proposals: &[String]) {
        self.index = 0;
        self.proposals.extend_from_slice(proposals)
    }

    /// Empty the proposals `Vec`.
    /// Reset the index.
    pub fn reset(&mut self) {
        self.index = 0;
        self.proposals.clear();
    }

    /// Goto completion.
    /// Looks for the valid path completing what the user typed.
    pub fn goto(&mut self, input_string: &str, current_path: &str) -> FmResult<()> {
        self.update_from_input(input_string, current_path);
        let (parent, last_name) = split_input_string(input_string);
        if last_name.is_empty() {
            return Ok(());
        }
        self.extend_absolute_paths(&parent, &last_name);
        self.extend_relative_paths(current_path, &last_name);
        Ok(())
    }

    fn update_from_input(&mut self, input_string: &str, current_path: &str) {
        if let Some(input_path) = self.canonicalize_input(input_string, current_path) {
            self.proposals = vec![input_path]
        } else {
            self.proposals = vec![]
        }
    }

    fn canonicalize_input(&mut self, input_string: &str, current_path: &str) -> Option<String> {
        let mut path = fs::canonicalize(current_path).unwrap();
        path.push(input_string);
        let path = fs::canonicalize(path).unwrap_or_default();
        if path.exists() {
            Some(path.to_str().unwrap_or_default().to_owned())
        } else {
            None
        }
    }

    fn extend_absolute_paths(&mut self, parent: &str, last_name: &str) {
        let Ok(path) = std::fs::canonicalize(parent) else { return };
        let Ok(entries) = fs::read_dir(path) else { return };
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
            .filter(|e| e.file_type().unwrap().is_dir() && filename_startswith(e, last_name))
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect()
    }

    /// Looks for programs in $PATH completing the one typed by the user.
    pub fn exec(&mut self, input_string: &str) -> FmResult<()> {
        let mut proposals: Vec<String> = vec![];
        if let Some(paths) = std::env::var_os("PATH") {
            for path in std::env::split_paths(&paths).filter(|path| path.exists()) {
                proposals.extend(Self::find_completion_in_path(path, input_string)?);
            }
        }
        self.update(proposals);
        Ok(())
    }

    pub fn command(&mut self, input_string: &str) -> FmResult<()> {
        let proposals = crate::action_map::ActionMap::iter()
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
    ) -> FmResult<Vec<String>> {
        Ok(fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().unwrap().is_file() && filename_startswith(e, input_string))
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect())
    }

    /// Looks for file within current folder completing what the user typed.
    pub fn search_from_normal(
        &mut self,
        input_string: &str,
        path_content: &PathContent,
    ) -> FmResult<()> {
        self.update(
            path_content
                .content
                .iter()
                .filter(|f| f.filename.contains(input_string))
                .map(|f| f.filename.clone())
                .collect(),
        );
        Ok(())
    }

    /// Looks for file within tree completing what the user typed.
    pub fn search_from_tree(
        &mut self,
        input_string: &str,
        content: &[(String, ColoredString)],
    ) -> FmResult<()> {
        self.update(
            content
                .iter()
                .filter(|(_, s)| s.text.contains(input_string))
                .map(|(_, s)| {
                    let text = s.text.replace("▸ ", "");
                    text.replace("▾ ", "")
                })
                .collect(),
        );

        Ok(())
    }

    /// Complete the input string with current_proposition if possible.
    /// Returns the optional last chars of the current_proposition.
    /// If the current_proposition doesn't start with input_string, it returns None.
    pub fn complete_input_string(&self, input_string: &str) -> Option<String> {
        let s = self.current_proposition();
        if s.starts_with(input_string) {
            Some(s[input_string.len()..].to_string())
        } else {
            None
        }
    }
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
    shellexpand::tilde(&parent).to_string()
}
