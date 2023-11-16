use std::collections::hash_map;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::iter::FilterMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::display_mode::files_collection;
use crate::display_mode::ContentWindow;
use crate::display_mode::Users;
use crate::display_mode::{ColorEffect, FileInfo};
use crate::display_mode::{ColoredTriplet, MakeTriplet};

use crate::common::filename_from_path;
use crate::edit_mode::FilterKind;
use crate::edit_mode::SortKind;

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
    reachable: bool,
    prev: PathBuf,
    next: PathBuf,
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
            reachable: true,
            next: PathBuf::default(),
            prev: PathBuf::default(),
        }
    }

    fn fold(&mut self) {
        self.folded = true
    }

    fn unfold(&mut self) {
        self.folded = false
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

    #[inline]
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
        if self.is_empty() {
            return;
        }
        match to {
            To::Next => self.select_next(),
            To::Prev => self.select_prev(),
            To::Root => self.select_root(),
            To::Last => self.select_last(),
            To::Parent => self.select_parent(),
            To::Path(path) => self.select_path(path, true),
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
    nodes: HashMap<PathBuf, Node>,
    required_height: usize,
}

impl Tree {
    pub const DEFAULT_REQUIRED_HEIGHT: usize = 80;

    /// Creates a new tree, exploring every node until depth is reached.
    pub fn new(
        root_path: PathBuf,
        depth: usize,
        sort_kind: SortKind,
        users: &Users,
        show_hidden: bool,
        filter_kind: &FilterKind,
    ) -> Self {
        let nodes = Self::make_nodes(
            &root_path,
            depth,
            sort_kind,
            users,
            show_hidden,
            filter_kind,
        );

        Self {
            selected: root_path.clone(),
            root_path,
            nodes,
            required_height: Self::DEFAULT_REQUIRED_HEIGHT,
        }
    }

    // TODO: refactor into small functions
    #[inline]
    fn make_nodes(
        root_path: &PathBuf,
        depth: usize,
        sort_kind: SortKind,
        users: &Users,
        show_hidden: bool,
        filter_kind: &FilterKind,
    ) -> HashMap<PathBuf, Node> {
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
            let children_will_be_added = depth + root_depth > 1 + reached_depth;
            let mut current_node = Node::new(&current_path, None);
            if children_will_be_added && current_path.is_dir() && !current_path.is_symlink() {
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

            if let Some(last_node) = nodes.get_mut(&last_path) {
                last_node.next = current_path.to_owned();
            }
            current_node.prev = last_path;
            nodes.insert(current_path.to_owned(), current_node);
            last_path = current_path.to_owned();
        }
        let Some(root_node) = nodes.get_mut(root_path) else {
            unreachable!("root_path should be in nodes");
        };
        root_node.prev = last_path.to_owned();
        root_node.select();
        let Some(last_node) = nodes.get_mut(&last_path) else {
            unreachable!("last_path should be in nodes");
        };
        last_node.next = root_path.to_owned();
        nodes
    }

    #[inline]
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
        if self.selected.is_dir() && !self.selected.is_symlink() {
            Some(self.selected.as_path())
        } else {
            self.selected.parent()
        }
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
    fn is_on_last(&self) -> bool {
        self.find_next_path() == self.root_path
    }

    /// Select next sibling or the next sibling of the parent
    fn select_next(&mut self) {
        let next_path = self.find_next_path();
        self.select_path(&next_path, false);
        self.increment_required_height()
    }

    fn find_next_path(&self) -> PathBuf {
        let mut current_path = self.selected.to_owned();
        loop {
            if let Some(current_node) = self.nodes.get(&current_path) {
                let next_path = &current_node.next;
                let Some(next_node) = self.nodes.get(next_path) else {
                    unreachable!("");
                };
                if next_node.reachable {
                    return next_path.to_owned();
                } else {
                    current_path = next_path.to_owned();
                }
            }
        }
    }

    /// Select previous sibling or the parent
    fn select_prev(&mut self) {
        let must_increase = self.is_on_root();
        let previous_path = self.find_prev_path();
        self.select_path(&previous_path, false);
        if must_increase {
            self.set_required_height_to_max()
        } else {
            self.decrement_required_height()
        }
    }

    fn find_prev_path(&self) -> PathBuf {
        let mut current_path = self.selected.to_owned();
        loop {
            if let Some(current_node) = self.nodes.get(&current_path) {
                let prev_path = &current_node.prev;
                let Some(prev_node) = self.nodes.get(prev_path) else {
                    unreachable!("");
                };
                if prev_node.reachable {
                    return prev_path.to_owned();
                } else {
                    current_path = prev_path.to_owned();
                }
            }
        }
    }

    pub fn page_up(&mut self) {
        for _ in 1..10 {
            if self.is_on_root() {
                break;
            }
            self.go(To::Prev);
        }
    }

    pub fn page_down(&mut self) {
        for _ in 1..10 {
            if self.is_on_last() {
                break;
            }
            self.go(To::Next);
        }
    }

    fn select_root(&mut self) {
        let root_path = self.root_path.to_owned();
        self.select_path(&root_path, false);
        self.reset_required_height()
    }

    fn select_last(&mut self) {
        self.select_root();
        self.select_prev();
    }

    fn select_parent(&mut self) {
        if let Some(parent_path) = self.selected.parent() {
            self.select_path(parent_path.to_owned().as_path(), false);
            self.decrement_required_height()
        }
    }

    fn select_path(&mut self, dest_path: &Path, set_height: bool) {
        if dest_path == self.selected {
            return;
        }
        let Some(dest_node) = self.nodes.get_mut(dest_path) else {
            return;
        };
        dest_node.select();
        let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
            unreachable!("current_node should be in nodes");
        };
        selected_node.unselect();
        self.selected = dest_path.to_owned();
        if set_height {
            self.set_required_height_to_max()
        }
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
            if node.folded {
                node.unfold();
                self.make_children_reachable()
            } else {
                node.fold();
                self.make_children_unreachable()
            }
        }
    }

    fn children_of_selected(&self) -> Vec<PathBuf> {
        self.nodes
            .keys()
            .filter(|p| p.starts_with(&self.selected) && p != &&self.selected)
            .map(|p| p.to_owned())
            .collect()
    }

    fn make_children_reachable(&mut self) {
        for path in self.children_of_selected().iter() {
            if let Some(child_node) = self.nodes.get_mut(path) {
                child_node.reachable = true;
                child_node.unfold();
            };
        }
    }

    fn make_children_unreachable(&mut self) {
        for path in self.children_of_selected().iter() {
            if let Some(child_node) = self.nodes.get_mut(path) {
                child_node.reachable = false;
            };
        }
    }

    /// Fold all node from root to end
    pub fn fold_all(&mut self) {
        for (_, node) in self.nodes.iter_mut() {
            node.fold()
        }
        self.select_root()
    }

    /// Unfold all node from root to end
    pub fn unfold_all(&mut self) {
        for (_, node) in self.nodes.iter_mut() {
            node.unfold()
        }
    }

    /// Select the first node whose filename match a pattern.
    /// If the selected file match, the next match will be selected.
    pub fn search_first_match(&mut self, pattern: &str) {
        let Some(found_path) = self.deep_first_search(pattern) else {
            return;
        };
        self.select_path(found_path.to_owned().as_path(), true);
    }

    fn deep_first_search(&self, pattern: &str) -> Option<PathBuf> {
        let mut stack = vec![self.root_path.as_path()];
        let mut found = vec![];

        while let Some(path) = stack.pop() {
            if path_filename_contains(path, pattern) {
                found.push(path.to_path_buf());
            }
            let Some(current_node) = self.nodes.get(path) else {
                continue;
            };

            if current_node.have_children() {
                let Some(children) = &current_node.children else {
                    continue;
                };
                for leaf in children.iter() {
                    stack.push(leaf);
                }
            }
        }
        self.pick_best_match(&found)
    }

    fn pick_best_match(&self, found: &[PathBuf]) -> Option<PathBuf> {
        if found.is_empty() {
            return None;
        }
        if let Some(position) = found.iter().position(|path| path == &self.selected) {
            // selected is in found
            if position + 1 < found.len() {
                // selected isn't last, use next elem
                Some(found[position + 1].to_owned())
            } else {
                // selected is last
                Some(found[0].to_owned())
            }
        } else {
            // selected isn't in found, use first match
            Some(found[0].to_owned())
        }
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

    /// An iterator over filenames.
    /// It allows us to iter explicitely over filenames
    /// while avoiding another allocation by collecting into a `Vec`
    #[inline]
    pub fn filenames(&self) -> Filenames<'_> {
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

type FnPbOsstr = fn(&PathBuf) -> Option<&OsStr>;
type FilterHashMap<'a> = FilterMap<hash_map::Keys<'a, PathBuf, Node>, FnPbOsstr>;
/// An iterator over filenames of a HashMap<PathBuf, Node>
pub type Filenames<'a> = FilterMap<FilterHashMap<'a>, fn(&OsStr) -> Option<&str>>;
