use std::fs;

use crate::{fileinfo::PathContent, fm_error::FmResult};

/// Holds a `Vec<String>` of possible completions and an `usize` index
/// showing where the user is in the vec.
#[derive(Clone)]
pub struct Completion {
    pub proposals: Vec<String>,
    pub index: usize,
}

impl Completion {
    /// Creates a new `Completion` instance with empty proposals and index=0.
    fn new() -> Self {
        Self {
            proposals: vec![],
            index: 0,
        }
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
    pub fn current_proposition(&self) -> String {
        if self.proposals.is_empty() {
            return "".to_owned();
        }
        self.proposals[self.index].to_owned()
    }

    /// Updates the proposition with a new `Vec`.
    /// Reset the index to 0.
    pub fn update(&mut self, proposals: Vec<String>) {
        self.index = 0;
        self.proposals = proposals;
    }

    /// Empty the proposals `Vec`.
    /// Reset the index.
    pub fn reset(&mut self) {
        self.index = 0;
        self.proposals.clear();
    }

    pub fn goto(&mut self, input_string: &str) -> FmResult<()> {
        let (parent, last_name) = split_input_string(input_string);
        if last_name.is_empty() {
            return Ok(());
        }
        if let Ok(path) = std::fs::canonicalize(parent) {
            if let Ok(entries) = fs::read_dir(path) {
                self.update(
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            e.file_type().unwrap().is_dir() && filename_startswith(e, &last_name)
                        })
                        .map(|e| e.path().to_string_lossy().into_owned())
                        .collect(),
                )
            }
        };
        Ok(())
    }

    pub fn exec(&mut self, input_string: &String) -> FmResult<()> {
        let mut proposals: Vec<String> = vec![];
        for path in std::env::var_os("PATH")
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .split(':')
            .filter(|path| std::path::Path::new(path).exists())
        {
            let comp: Vec<String> = fs::read_dir(path)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().unwrap().is_file() && filename_startswith(e, input_string)
                })
                .map(|e| e.path().to_string_lossy().into_owned())
                .collect();
            proposals.extend(comp);
        }
        self.update(proposals);
        Ok(())
    }

    pub fn search(&mut self, input_string: &String, path_content: &PathContent) -> FmResult<()> {
        self.update(
            path_content
                .files
                .iter()
                .filter(|f| f.filename.contains(input_string))
                .map(|f| f.filename.clone())
                .collect(),
        );
        Ok(())
    }
}

impl Default for Completion {
    fn default() -> Self {
        Self::new()
    }
}

/// true if the filename starts with a pattern
fn filename_startswith(entry: &std::fs::DirEntry, pattern: &String) -> bool {
    entry
        .file_name()
        .to_string_lossy()
        .into_owned()
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
