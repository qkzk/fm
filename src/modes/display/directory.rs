use std::borrow::Borrow;
use std::collections::{BTreeSet, VecDeque};
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::app::TabSettings;
use crate::io::git;
use crate::modes::{is_not_hidden, path_is_video, FileInfo, FileKind, FilterKind, SortKind, Users};
use crate::{impl_content, impl_index_to_index, impl_selectable, log_info};

/// Holds the information about file in the current directory.
/// We know about the current path, the files themselves, the selected index,
/// the "display all files including hidden" flag and the key to sort files.
pub struct Directory {
    /// The current path
    pub path: Arc<Path>,
    /// A vector of FileInfo with every file in current path
    pub content: Vec<FileInfo>,
    /// The index of the selected file.
    pub index: usize,
    used_space: u64,
}

impl Directory {
    /// Reads the paths and creates a new `PathContent`.
    /// Files are sorted by filename by default.
    /// Selects the first file if any.
    pub fn new(path: &Path, users: &Users, filter: &FilterKind, show_hidden: bool) -> Result<Self> {
        let path = Arc::from(path);
        let mut content = Self::files(&path, show_hidden, filter, users)?;
        let sort_kind = SortKind::default();
        sort_kind.sort(&mut content);
        let index: usize = 0;
        let used_space = get_used_space(&content);

        Ok(Self {
            path,
            content,
            index,
            used_space,
        })
    }

    pub fn change_directory(
        &mut self,
        path: &Path,
        settings: &TabSettings,
        users: &Users,
    ) -> Result<()> {
        self.content = Self::files(path, settings.show_hidden, &settings.filter, users)?;
        settings.sort_kind.sort(&mut self.content);
        self.index = 0;
        self.used_space = get_used_space(&self.content);
        self.path = Arc::from(path);
        Ok(())
    }

    fn files(
        path: &Path,
        show_hidden: bool,
        filter_kind: &FilterKind,
        users: &Users,
    ) -> Result<Vec<FileInfo>> {
        let mut files: Vec<FileInfo> = Self::create_dot_dotdot(path, users)?;
        if let Some(true_files) = files_collection(path, users, show_hidden, filter_kind, false) {
            files.extend(true_files);
        }
        Ok(files)
    }

    fn create_dot_dotdot(path: &Path, users: &Users) -> Result<Vec<FileInfo>> {
        let current = FileInfo::from_path_with_name(path, ".", users)?;
        let Some(parent) = path.parent() else {
            return Ok(vec![current]);
        };
        let parent = FileInfo::from_path_with_name(parent, "..", users)?;
        Ok(vec![current, parent])
    }

    /// Sort the file with current key.
    pub fn sort(&mut self, sort_kind: &SortKind) {
        sort_kind.sort(&mut self.content)
    }

    /// Calculates the size of the owner column.
    pub fn owner_column_width(&self) -> usize {
        let owner_size_btreeset: BTreeSet<usize> =
            self.iter().map(|file| file.owner.len()).collect();
        *owner_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Calculates the size of the group column.
    pub fn group_column_width(&self) -> usize {
        let group_size_btreeset: BTreeSet<usize> =
            self.iter().map(|file| file.group.len()).collect();
        *group_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Select the file from a given index.
    pub fn select_index(&mut self, index: usize) {
        if index < self.content.len() {
            self.index = index;
        }
    }

    /// Reset the current file content.
    /// Reads and sort the content with current key.
    /// Select the first file if any.
    pub fn reset_files(&mut self, settings: &TabSettings, users: &Users) -> Result<()> {
        self.content = Self::files(&self.path, settings.show_hidden, &settings.filter, users)?;
        self.sort(&settings.sort_kind);
        self.index = 0;
        Ok(())
    }

    /// Is the selected file a directory ?
    /// It may fails if the current path is empty, aka if nothing is selected.
    pub fn is_selected_dir(&self) -> Result<bool> {
        let fileinfo = self.selected().context("")?;
        match fileinfo.file_kind {
            FileKind::Directory => Ok(true),
            FileKind::SymbolicLink(true) => {
                let dest = std::fs::read_link(&fileinfo.path).unwrap_or_default();
                Ok(dest.is_dir())
            }
            _ => Ok(false),
        }
    }

    /// Human readable string representation of the space used by _files_
    /// in current path.
    /// No recursive exploration of directory.
    pub fn used_space(&self) -> String {
        human_size(self.used_space)
    }

    /// A string representation of the git status of the path.
    pub fn git_string(&self) -> Result<String> {
        git(&self.path)
    }

    /// Returns an iterator of the files (`FileInfo`) in content.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, FileInfo> {
        self.content.iter()
    }

    /// Returns an enumeration of the files (`FileInfo`) in content.
    #[inline]
    pub fn enumerate(&self) -> Enumerate<std::slice::Iter<'_, FileInfo>> {
        self.iter().enumerate()
    }

    /// Returns the correct index jump target to a flagged files.
    fn find_jump_index(&self, jump_target: &Path) -> Option<usize> {
        self.content
            .iter()
            .position(|file| <Arc<Path> as Borrow<Path>>::borrow(&file.path) == jump_target)
    }

    /// Select the file from its path. Returns its index in content.
    pub fn select_file(&mut self, jump_target: &Path) -> usize {
        let index = self.find_jump_index(jump_target).unwrap_or_default();
        self.select_index(index);
        index
    }

    /// Returns a vector of paths from content
    pub fn paths(&self) -> Vec<&Path> {
        self.content
            .iter()
            .map(|fileinfo| fileinfo.path.borrow())
            .collect()
    }

    /// True iff the selected path is ".." which is the parent dir.
    pub fn is_dotdot_selected(&self) -> bool {
        let Some(selected) = &self.selected() else {
            return false;
        };
        let Some(parent) = self.path.parent() else {
            return false;
        };
        selected.path.as_ref() == parent
    }

    pub fn videos(&self) -> VecDeque<PathBuf> {
        self.content()
            .iter()
            .map(|f| f.path.to_path_buf())
            .filter(|p| path_is_video(p))
            .collect()
    }
}

impl_index_to_index!(FileInfo, Directory);
impl_selectable!(Directory);
impl_content!(Directory, FileInfo);

fn get_used_space(files: &[FileInfo]) -> u64 {
    files
        .iter()
        .filter(|f| !f.is_dir())
        .map(|f| f.true_size)
        .sum()
}

/// Creates an optional vector of fileinfo contained in a file.
/// Files are filtered by filterkind and the display hidden flag.
/// Returns None if there's no file.
pub fn files_collection(
    path: &Path,
    users: &Users,
    show_hidden: bool,
    filter_kind: &FilterKind,
    keep_dir: bool,
) -> Option<Vec<FileInfo>> {
    match read_dir(path) {
        Ok(read_dir) => Some(
            read_dir
                .filter_map(|direntry| direntry.ok())
                .filter(|direntry| show_hidden || is_not_hidden(direntry).unwrap_or(true))
                .filter_map(|direntry| FileInfo::from_direntry(&direntry, users).ok())
                .filter(|fileinfo| filter_kind.filter_by(fileinfo, keep_dir))
                .collect(),
        ),
        Err(error) => {
            log_info!("Couldn't read path {path} - {error}", path = path.display(),);
            None
        }
    }
}

const SIZES: [&str; 9] = ["B", "k", "M", "G", "T", "P", "E", "Z", "Y"];

/// Formats a file size from bytes to human readable string.
#[inline]
pub fn human_size(bytes: u64) -> String {
    let mut factor = 0;
    let mut size = bytes as f64;

    while size >= 1000.0 && factor < SIZES.len() - 1 {
        size /= 1000.0;
        factor += 1;
    }

    if size < 9.5 && factor > 0 {
        format!("{size:.1}{unit}", unit = SIZES[factor])
    } else {
        format!(
            "{size:>3}{unit}",
            size = size.round() as u64,
            unit = SIZES[factor]
        )
    }
}
