use std::borrow::Borrow;
use std::fs::read_dir;
use std::iter::Enumerate;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::{Context, Result};

use crate::app::TabSettings;
use crate::common::{filename_from_path, path_to_string};
use crate::io::git;
use crate::modes::FilterKind;
use crate::modes::SortKind;
use crate::modes::Users;
use crate::modes::{is_not_hidden, FileInfo, FileKind};
use crate::{impl_selectable_content, log_info};

/// Holds the information about file in the current directory.
/// We know about the current path, the files themselves, the selected index,
/// the "display all files including hidden" flag and the key to sort files.
pub struct Directory {
    /// The current path
    pub path: PathBuf,
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
        let path = path.to_owned();
        let mut content = Self::files(&path, show_hidden, filter, users)?;
        let sort_kind = SortKind::default();
        sort_kind.sort(&mut content);
        let selected_index: usize = 0;
        if !content.is_empty() {
            content[selected_index].select();
        }
        let used_space = get_used_space(&content);

        Ok(Self {
            path,
            content,
            index: selected_index,
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
        if !self.content.is_empty() {
            self.content[0].select()
        }
        self.used_space = get_used_space(&self.content);
        self.path = path.to_path_buf();
        Ok(())
    }

    fn files(
        path: &Path,
        show_hidden: bool,
        filter_kind: &FilterKind,
        users: &Users,
    ) -> Result<Vec<FileInfo>> {
        let mut files: Vec<FileInfo> = Self::create_dot_dotdot(path, users)?;

        let fileinfo = FileInfo::from_path_with_name(path, filename_from_path(path)?, users)?;
        if let Some(true_files) =
            files_collection(&fileinfo.path, users, show_hidden, filter_kind, false)
        {
            files.extend(true_files);
        }
        Ok(files)
    }

    fn create_dot_dotdot(path: &Path, users: &Users) -> Result<Vec<FileInfo>> {
        let current = FileInfo::from_path_with_name(path, ".", users)?;
        match path.parent() {
            Some(parent) => {
                let parent = FileInfo::from_path_with_name(parent, "..", users)?;
                Ok(vec![current, parent])
            }
            None => Ok(vec![current]),
        }
    }

    /// Convert a path to a &str.
    /// It may fails if the path contains non valid utf-8 chars.
    pub fn path_to_str(&self) -> String {
        path_to_string(&self.path)
    }

    /// Sort the file with current key.
    pub fn sort(&mut self, sort_kind: &SortKind) {
        sort_kind.sort(&mut self.content)
    }

    /// Calculates the size of the owner column.
    pub fn owner_column_width(&self) -> usize {
        let owner_size_btreeset: std::collections::BTreeSet<usize> =
            self.iter().map(|file| file.owner.len()).collect();
        *owner_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Calculates the size of the group column.
    pub fn group_column_width(&self) -> usize {
        let group_size_btreeset: std::collections::BTreeSet<usize> =
            self.iter().map(|file| file.group.len()).collect();
        *group_size_btreeset.iter().next_back().unwrap_or(&1)
    }

    /// Select the file from a given index.
    pub fn select_index(&mut self, index: usize) {
        if index < self.content.len() {
            self.unselect_current();
            self.content[index].select();
            self.index = index;
        }
    }

    /// Reset the current file content.
    /// Reads and sort the content with current key.
    /// Select the first file if any.
    pub fn reset_files(&mut self, settings: &TabSettings, users: &Users) -> Result<()> {
        self.content = Self::files(&self.path, settings.show_hidden, &settings.filter, users)?;
        let sort_kind = SortKind::default();
        self.sort(&sort_kind);
        self.index = 0;
        if !self.content.is_empty() {
            self.content[self.index].select();
        }
        Ok(())
    }

    /// True if the path starts with a subpath.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.path)
    }

    /// Is the selected file a directory ?
    /// It may fails if the current path is empty, aka if nothing is selected.
    pub fn is_selected_dir(&self) -> Result<bool> {
        match self
            .selected()
            .context("is selected dir: Empty directory")?
            .file_kind
        {
            FileKind::Directory => Ok(true),
            FileKind::SymbolicLink(true) => {
                let dest = read_symlink_dest(
                    &self
                        .selected()
                        .context("is selected dir: unreachable")?
                        .path,
                )
                .unwrap_or_default();
                Ok(PathBuf::from(dest).is_dir())
            }
            FileKind::SymbolicLink(false) => Ok(false),
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

    /// Unselect the current item.
    /// Since we use a common trait to navigate the files,
    /// this method is required.
    pub fn unselect_current(&mut self) {
        if self.is_empty() {
            return;
        }
        self.content[self.index].unselect();
    }

    /// Select the current item.
    /// Since we use a common trait to navigate the files,
    /// this method is required.
    pub fn select_current(&mut self) {
        if self.is_empty() {
            return;
        }
        self.content[self.index].select();
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
            .position(|file| <Rc<Path> as Borrow<Path>>::borrow(&file.path) == jump_target)
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

    pub fn filenames_containing(&self, input_string: &str) -> Vec<String> {
        self.content
            .iter()
            .filter(|f| f.filename.contains(input_string))
            .map(|f| f.filename.to_string())
            .collect()
    }
}

impl_selectable_content!(FileInfo, Directory);

const MAX_PATH_ELEM_SIZE: usize = 50;

/// Shorten a path to be displayed in [`MAX_PATH_ELEM_SIZE`] chars or less.
/// Each element of the path is shortened if needed.
pub fn shorten_path(path: &Path, size: Option<usize>) -> Result<String> {
    if path == Path::new("/") {
        return Ok("".to_owned());
    }
    let size = match size {
        Some(size) => size,
        None => MAX_PATH_ELEM_SIZE,
    };
    let path_string = path
        .to_str()
        .context("summarize: couldn't parse the path")?
        .to_owned();

    if path_string.len() < size {
        return Ok(path_string);
    }

    let splitted_path: Vec<_> = path_string.split('/').collect();
    let size_per_elem = std::cmp::max(1, size / (splitted_path.len() + 1)) + 1;
    let shortened_elems: Vec<_> = splitted_path
        .iter()
        .filter_map(|p| {
            if p.len() <= size_per_elem {
                Some(*p)
            } else {
                p.get(0..size_per_elem)
            }
        })
        .collect();
    Ok(shortened_elems.join("/"))
}

/// Returns `Some(destination)` where `destination` is a String if the path is
/// the destination of a symlink,
/// Returns `None` if the link is broken, if the path doesn't exists or if the path
/// isn't a symlink.
pub fn read_symlink_dest(path: &Path) -> Option<String> {
    match std::fs::read_link(path) {
        Ok(dest) if dest.exists() => Some(dest.to_str()?.to_owned()),
        _ => None,
    }
}

fn get_used_space(files: &[FileInfo]) -> u64 {
    files.iter().map(|f| f.true_size).sum()
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
                .map(|direntry| FileInfo::from_direntry(&direntry, users))
                .filter_map(|fileinfo| fileinfo.ok())
                .filter(|fileinfo| filter_kind.filter_by(fileinfo, keep_dir))
                .collect(),
        ),
        Err(error) => {
            log_info!("Couldn't read path {path} - {error}", path = path.display(),);
            None
        }
    }
}

/// Convert a file size from bytes to human readable string.
pub fn human_size(bytes: u64) -> String {
    let size = ["", "k", "M", "G", "T", "P", "E", "Z", "Y"];
    let factor = (bytes.to_string().chars().count() as u64 - 1) / 3_u64;
    format!(
        "{:>3}{:<1}",
        bytes / (1024_u64).pow(factor as u32),
        size[factor as usize]
    )
}
