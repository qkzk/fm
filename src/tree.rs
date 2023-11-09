use std::collections::hash_map;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::iter::FilterMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::content_window::ContentWindow;
use crate::fileinfo::{files_collection, ColorEffect, FileInfo};
use crate::filter::FilterKind;
use crate::preview::{ColoredTriplet, MakeTriplet};
use crate::sort::SortKind;
use crate::users::Users;
use crate::utils::filename_from_path;

/// Holds a string, its display attributes and the associated pathbuf.
#[derive(Clone, Debug)]
pub struct ColoredString {
    /// A text to be printed. In most case, it should be a filename.
    pub text: String,
    /// A tuikit::attr::Attr (fg, bg, effect) to enhance the text.
    pub color_effect: ColorEffect,
    /// The complete path of this string.
    pub path: std::path::PathBuf,
}

impl ColoredString {
    /// Creates a new colored string.
    pub fn new(text: String, color_effect: ColorEffect, path: std::path::PathBuf) -> Self {
        Self {
            text,
            color_effect,
            path,
        }
    }
}

/// An element of a tree.
/// It's a file/directory, some optional children.
/// A Node knows if it's folded or selected.
#[derive(Debug, Clone)]
pub struct Node {
    path: PathBuf,
    children: Option<Vec<PathBuf>>,
    folded: bool,
    selected: bool,
}

impl Node {
    /// Creates a new Node from a path and its children.
    /// By default it's not selected nor folded.
    fn new(path: &Path, children: Option<Vec<PathBuf>>) -> Self {
        Self {
            path: path.to_owned(),
            children,
            folded: false,
            selected: false,
        }
    }

    fn fold(&mut self) {
        self.folded = true
    }

    fn unfold(&mut self) {
        self.folded = false
    }

    fn toggle_fold(&mut self) {
        self.folded = !self.folded
    }

    fn select(&mut self) {
        self.selected = true
    }

    fn unselect(&mut self) {
        self.selected = false
    }

    /// Is the node selected ?
    pub fn selected(&self) -> bool {
        self.selected
    }

    /// Creates a new fileinfo from the node.
    pub fn fileinfo(&self, users: &Users) -> Result<FileInfo> {
        FileInfo::new(&self.path, users)
    }

    fn set_children(&mut self, children: Option<Vec<PathBuf>>) {
        self.children = children
    }

    #[inline]
    fn have_children(self: &Node) -> bool {
        !self.folded && self.children.is_some()
    }
}

/// Describe a movement in a navigable structure
pub trait Go {
    fn go(&mut self, to: To);
}

/// Describes a direction for the next selected tree element.
pub enum To<'a> {
    Next,
    Prev,
    Root,
    Last,
    Parent,
    Path(&'a Path),
}

impl Go for Tree {
    /// Select another element from a tree.
    fn go(&mut self, to: To) {
        match to {
            To::Next => self.select_next(),
            To::Prev => self.select_prev(),
            To::Root => self.select_root(),
            To::Last => self.select_last(),
            To::Parent => self.select_parent(),
            To::Path(path) => self.select_path(path),
        }
    }
}

/// A FileSystem tree of nodes.
/// Internally it's a wrapper around an `std::collections::HashMap<PathBuf, Node>`
/// It also holds informations about the required height of the tree.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    root_path: PathBuf,
    selected: PathBuf,
    last_path: PathBuf,
    nodes: HashMap<PathBuf, Node>,
    required_height: usize,
}

impl Tree {
    pub const DEFAULT_REQUIRED_HEIGHT: usize = 80;

    /// Creates a new tree, exploring every node untill depth is reached.
    pub fn new(
        root_path: PathBuf,
        depth: usize,
        sort_kind: SortKind,
        users: &Users,
        show_hidden: bool,
        filter_kind: &FilterKind,
    ) -> Self {
        // keep track of the depth
        let root_depth = root_path.components().collect::<Vec<_>>().len();
        let mut stack = vec![root_path.to_owned()];
        let mut nodes: HashMap<PathBuf, Node> = HashMap::new();
        let mut last_path = root_path.to_owned();

        while let Some(current_path) = stack.pop() {
            let reached_depth = current_path.components().collect::<Vec<_>>().len();
            if reached_depth >= depth + root_depth {
                continue;
            }
            let children_will_be_added = depth + root_depth - reached_depth > 1;
            let mut current_node = Node::new(&current_path, None);
            if current_path.is_dir() && !current_path.is_symlink() && children_will_be_added {
                if let Some(mut files) =
                    files_collection(&current_path, users, show_hidden, filter_kind, true)
                {
                    sort_kind.sort(&mut files);
                    let children = Self::make_children_and_stack_them(&mut stack, &files);
                    if !children.is_empty() {
                        current_node.set_children(Some(children));
                    }
                };
            }
            last_path = current_path.to_owned();
            nodes.insert(current_path.to_owned(), current_node);
        }

        let Some(root_node) = nodes.get_mut(&root_path) else {
            unreachable!("root path should be in nodes");
        };
        root_node.select();

        Self {
            selected: root_path.clone(),
            root_path,
            last_path,
            nodes,
            required_height: Self::DEFAULT_REQUIRED_HEIGHT,
        }
    }

    fn make_children_and_stack_them(stack: &mut Vec<PathBuf>, files: &[FileInfo]) -> Vec<PathBuf> {
        files
            .iter()
            .map(|fileinfo| fileinfo.path.to_owned())
            .map(|path| {
                stack.push(path.to_owned());
                path
            })
            .collect()
    }

    /// Root path of the tree.
    pub fn root_path(&self) -> &Path {
        self.root_path.as_path()
    }

    /// Selected path
    pub fn selected_path(&self) -> &Path {
        self.selected.as_path()
    }

    /// Selected node
    pub fn selected_node(&self) -> Option<&Node> {
        self.nodes.get(&self.selected)
    }

    /// The folder containing the selected node.
    /// Itself if selected is a directory.
    pub fn directory_of_selected(&self) -> Option<&Path> {
        let ret = if self.selected.is_dir() && !self.selected.is_symlink() {
            Some(self.selected.as_path())
        } else {
            self.selected.parent()
        };
        log::info!("directory_of_selected {ret:?}");
        ret
    }

    /// Relative path of selected from rootpath.
    pub fn selected_path_relative_to_root(&self) -> Result<&Path> {
        Ok(self.selected.strip_prefix(&self.root_path)?)
    }

    /// Number of nodes
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True if there's no node.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// True if selected is root.
    pub fn is_on_root(&self) -> bool {
        self.selected == self.root_path
    }

    /// True if selected is the last file
    pub fn is_on_last(&self) -> bool {
        self.selected == self.last_path
    }

    /// Select next sibling or the next sibling of the parent
    fn select_next(&mut self) {
        if self.is_on_last() {
            self.select_root();
            return;
        }

        if let Some(next_path) = self.find_next_path() {
            let Some(next_node) = self.nodes.get_mut(&next_path) else {
                return;
            };
            next_node.select();
            let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
                unreachable!("current_node should be in nodes");
            };
            selected_node.unselect();
            self.selected = next_path;
            self.increment_required_height()
        }
    }

    // FIX: Still a problem when reaching max depth of tree,
    // can't find next sibling since we look for children which exists but aren't in tree.
    // Check if the children are listed (they shouldn't be) in node.children and are in self.nodes.
    fn find_next_path(&self) -> Option<PathBuf> {
        let Some(current_node) = self.nodes.get(&self.selected) else {
            unreachable!("selected path should be in nodes")
        };
        if !self.selected.is_dir() || !current_node.folded {
            if let Some(children_paths) = &current_node.children {
                if let Some(child_path) = children_paths.last() {
                    let child_path = child_path.to_owned();
                    return Some(child_path.to_owned());
                }
            }
        }
        let mut current_path = self.selected.to_owned();

        // TODO: refactor using ancestors. Not so easy since we keep track of parent and current
        while let Some(parent_path) = current_path.parent() {
            let Some(parent_node) = self.nodes.get(parent_path) else {
                current_path = parent_path.to_owned();
                continue;
            };
            let Some(siblings_paths) = &parent_node.children else {
                current_path = parent_path.to_owned();
                continue;
            };
            let Some(index_current) = siblings_paths.iter().position(|path| path == &current_path)
            else {
                current_path = parent_path.to_owned();
                continue;
            };
            if index_current == 0 {
                current_path = parent_path.to_owned();
                continue;
            }
            let Some(next_sibling_path) = siblings_paths.get(index_current - 1) else {
                current_path = parent_path.to_owned();
                continue;
            };
            if self.nodes.contains_key(next_sibling_path) {
                return Some(next_sibling_path.to_owned());
            } else {
                current_path = parent_path.to_owned();
                continue;
            };
        }
        None
    }
    // TODO! find the bottom child of parent instead of jumping back 1 level
    /// Select previous sibling or the parent
    fn select_prev(&mut self) {
        if self.is_on_root() {
            self.select_last();
            return;
        }

        if let Some(previous_path) = self.find_prev_path() {
            let Some(previous_node) = self.nodes.get_mut(&previous_path) else {
                return;
            };
            previous_node.select();
            let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
                unreachable!("current_node should be in nodes");
            };
            selected_node.unselect();
            self.selected = previous_path;
            self.decrement_required_height()
        }
    }

    fn find_prev_path(&self) -> Option<PathBuf> {
        let current_path = self.selected.to_owned();
        let Some(parent_path) = current_path.parent() else {
            return None;
        };
        let Some(parent_node) = self.nodes.get(parent_path) else {
            return None;
        };
        let Some(siblings_paths) = &parent_node.children else {
            return None;
        };
        let Some(index_current) = siblings_paths.iter().position(|path| path == &current_path)
        else {
            return None;
        };
        if index_current + 1 < siblings_paths.len() {
            Some(siblings_paths[index_current + 1].to_owned())
        } else {
            let Some(_node) = self.nodes.get(parent_path) else {
                return None;
            };
            Some(parent_path.to_owned())
        }
    }

    fn select_root(&mut self) {
        let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
            unreachable!("selected path should be in node")
        };
        selected_node.unselect();
        let Some(root_node) = self.nodes.get_mut(&self.root_path) else {
            unreachable!("root path should be in nodes")
        };
        root_node.select();
        self.selected = self.root_path.to_owned();
        self.reset_required_height()
    }

    fn select_last(&mut self) {
        let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
            unreachable!("selected path should be in node")
        };
        selected_node.unselect();
        let Some(last_node) = self.nodes.get_mut(&self.last_path) else {
            unreachable!("root path should be in nodes")
        };
        last_node.select();
        self.selected = self.last_path.to_owned();
        self.set_required_height_to_max()
    }

    fn select_parent(&mut self) {
        if let Some(parent_path) = self.selected.parent() {
            let Some(parent_node) = self.nodes.get_mut(parent_path) else {
                return;
            };
            parent_node.select();
            let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
                unreachable!("current_node should be in nodes");
            };
            selected_node.unselect();
            self.selected = parent_path.to_owned();
            self.decrement_required_height()
        }
    }

    fn select_path(&mut self, clicked_path: &Path) {
        if clicked_path == self.selected {
            return;
        }
        let Some(new_node) = self.nodes.get_mut(clicked_path) else {
            return;
        };
        new_node.select();
        let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
            unreachable!("current_node should be in nodes");
        };
        selected_node.unselect();
        self.selected = clicked_path.to_owned();
        self.set_required_height_to_max()
    }

    fn increment_required_height(&mut self) {
        if self.required_height < usize::MAX {
            self.required_height += 1
        }
    }

    fn decrement_required_height(&mut self) {
        if self.required_height > Self::DEFAULT_REQUIRED_HEIGHT {
            self.required_height -= 1
        }
    }

    fn set_required_height_to_max(&mut self) {
        self.required_height = usize::MAX
    }

    fn reset_required_height(&mut self) {
        self.required_height = Self::DEFAULT_REQUIRED_HEIGHT
    }

    /// Fold selected node
    pub fn toggle_fold(&mut self) {
        if let Some(node) = self.nodes.get_mut(&self.selected) {
            node.toggle_fold();
        }
    }

    /// Fold all node from root to end
    pub fn fold_all(&mut self) {
        for (_, node) in self.nodes.iter_mut() {
            node.fold()
        }
    }

    /// Unfold all node from root to end
    pub fn unfold_all(&mut self) {
        for (_, node) in self.nodes.iter_mut() {
            node.unfold()
        }
    }

    /// Select the first node whose filename match a pattern.
    pub fn search_first_match(&mut self, pattern: &str) {
        let Some(current_index) = self.nodes.keys().position(|path| path == &self.selected) else {
            unreachable!("selected should be in pos");
        };
        if let Some(found_path) = self
            .nodes
            .keys()
            .skip(current_index + 1)
            .find(|path| path_filename_contains(path, pattern))
        {
            self.go(To::Path(found_path.to_owned().as_path()));
        } else if let Some(found_path) = self
            .nodes
            .keys()
            .find(|path| path_filename_contains(path, pattern))
        {
            self.go(To::Path(found_path.to_owned().as_path()));
        };
    }

    /// Returns a navigable vector of `ColoredTriplet` and the index of selected file
    pub fn into_navigable_content(&self, users: &Users) -> (usize, Vec<ColoredTriplet>) {
        let mut stack = vec![("".to_owned(), self.root_path.as_path())];
        let mut content = vec![];
        let mut selected_index = 0;

        while let Some((prefix, path)) = stack.pop() {
            let Some(node) = self.nodes.get(path) else {
                continue;
            };

            if node.selected {
                selected_index = content.len();
            }

            let Ok(fileinfo) = FileInfo::new(path, users) else {
                continue;
            };

            content.push(<ColoredTriplet as MakeTriplet>::make(
                &fileinfo,
                &prefix,
                filename_format(path, node),
                ColorEffect::node(&fileinfo, node),
                path,
            ));

            if node.have_children() {
                Self::stack_children(&mut stack, prefix, node);
            }

            if content.len() > self.required_height {
                break;
            }
        }
        (selected_index, content)
    }

    /// An (ugly) iterator over filenames.
    /// It allows us to iter explicitely over filenames
    /// while avoiding another allocation by collecting into a `Vec`
    #[inline]
    pub fn filenames(
        &self,
    ) -> FilterMap<
        FilterMap<hash_map::Keys<'_, PathBuf, Node>, fn(&PathBuf) -> Option<&OsStr>>,
        fn(&OsStr) -> Option<&str>,
    > {
        let to_filename: fn(&PathBuf) -> Option<&OsStr> = |path| path.file_name();
        let to_str: fn(&OsStr) -> Option<&str> = |filename| filename.to_str();
        self.nodes.keys().filter_map(to_filename).filter_map(to_str)
    }

    #[inline]
    fn stack_children<'a>(
        stack: &mut Vec<(String, &'a Path)>,
        prefix: String,
        current_node: &'a Node,
    ) {
        let first_prefix = first_prefix(prefix.clone());
        let other_prefix = other_prefix(prefix);

        let Some(children) = &current_node.children else {
            return;
        };
        let mut children = children.iter();
        let Some(first_leaf) = children.next() else {
            return;
        };
        stack.push((first_prefix.clone(), first_leaf));

        for leaf in children {
            stack.push((other_prefix.clone(), leaf));
        }
    }
}

#[inline]
fn first_prefix(mut prefix: String) -> String {
    prefix.push(' ');
    prefix = prefix.replace("└──", "  ");
    prefix = prefix.replace("├──", "│ ");
    prefix.push_str("└──");
    prefix
}

#[inline]
fn other_prefix(mut prefix: String) -> String {
    prefix.push(' ');
    prefix = prefix.replace("└──", "  ");
    prefix = prefix.replace("├──", "│ ");
    prefix.push_str("├──");
    prefix
}

#[inline]
fn filename_format(current_path: &Path, current_node: &Node) -> String {
    let filename = filename_from_path(current_path)
        .unwrap_or_default()
        .to_owned();

    if current_path.is_dir() && !current_path.is_symlink() {
        if current_node.folded {
            format!("▸ {}", filename)
        } else {
            format!("▾ {}", filename)
        }
    } else {
        filename
    }
}

/// Emulate a `ContentWindow`, returning the top and bottom index of displayable files.
pub fn calculate_top_bottom(selected_index: usize, terminal_height: usize) -> (usize, usize) {
    let window_height = terminal_height - ContentWindow::WINDOW_MARGIN_TOP;
    let top = if selected_index < window_height {
        0
    } else {
        selected_index - 10.max(terminal_height / 2)
    };
    let bottom = top + window_height;

    (top, bottom)
}

fn path_filename_contains(path: &Path, pattern: &str) -> bool {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .contains(pattern)
}
