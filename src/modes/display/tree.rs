use std::borrow::Borrow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::rc::Rc;

use anyhow::Result;

use crate::common::filename_from_path;
use crate::common::has_last_modification_happened_less_than;
use crate::modes::files_collection;
use crate::modes::ContentWindow;
use crate::modes::FilterKind;
use crate::modes::SortKind;
use crate::modes::Users;
use crate::modes::{ColorEffect, FileInfo};
use crate::modes::{ColoredTriplet, MakeTriplet};

/// Holds a string, its display attributes and the associated pathbuf.
#[derive(Clone, Debug)]
pub struct ColoredString {
    /// A text to be printed. In most case, it should be a filename.
    pub text: String,
    /// A pair of [`tuikit::attr::Color`] and [`tuikit::attr::Effect`] used to enhance the text.
    pub color_effect: ColorEffect,
    /// The complete path of this string.
    pub path: Rc<Path>,
}

impl ColoredString {
    /// Creates a new colored string.
    pub fn new(text: String, color_effect: ColorEffect, path: Rc<Path>) -> Self {
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
    path: Rc<Path>,
    prev: Rc<Path>,
    next: Rc<Path>,
    index: usize,
    children: Option<Vec<Rc<Path>>>,
    folded: bool,
    selected: bool,
    reachable: bool,
}

impl Node {
    /// Creates a new Node from a path and its children.
    /// By default it's not selected nor folded.
    fn new(path: &Path, children: Option<Vec<Rc<Path>>>, prev: &Path, index: usize) -> Self {
        Self {
            path: Rc::from(path),
            prev: Rc::from(prev),
            next: Rc::from(Path::new("")),
            index,
            children,
            folded: false,
            selected: false,
            reachable: true,
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

    /// The index of the node in displayed tree
    pub fn index(&self) -> usize {
        self.index
    }

    /// Creates a new fileinfo from the node.
    pub fn fileinfo(&self, users: &Users) -> Result<FileInfo> {
        FileInfo::new(&self.path, users)
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

/// Trait allowing to measure the depth of something.
trait Depth {
    fn depth(&self) -> usize;
}

impl Depth for Rc<Path> {
    /// Measure the number of components of a `PathBuf`.
    /// For absolute paths, it's the number of folders plus one for / and one for the file itself.
    #[inline]
    fn depth(&self) -> usize {
        self.components().collect::<Vec<_>>().len()
    }
}

/// A FileSystem tree of nodes.
/// Internally it's a wrapper around an `std::collections::HashMap<PathBuf, Node>`
/// It also holds informations about the required height of the tree.
#[derive(Debug, Clone)]
pub struct Tree {
    root_path: Rc<Path>,
    selected: Rc<Path>,
    nodes: HashMap<Rc<Path>, Node>,
    required_height: usize,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            root_path: Rc::from(Path::new("")),
            selected: Rc::from(Path::new("")),
            nodes: HashMap::new(),
            required_height: 0,
        }
    }
}

impl Tree {
    pub const DEFAULT_REQUIRED_HEIGHT: usize = 80;

    /// Creates a new tree, exploring every node until depth is reached.
    pub fn new(
        root_path: Rc<Path>,
        depth: usize,
        sort_kind: SortKind,
        users: &Users,
        show_hidden: bool,
        filter_kind: &FilterKind,
    ) -> Self {
        let nodes = Self::make_nodes(
            root_path.clone(),
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

    #[inline]
    fn make_nodes(
        root_path: Rc<Path>,
        depth: usize,
        sort_kind: SortKind,
        users: &Users,
        show_hidden: bool,
        filter_kind: &FilterKind,
    ) -> HashMap<Rc<Path>, Node> {
        // keep track of the depth
        let root_depth = root_path.depth();
        let mut stack = vec![root_path.to_owned()];
        let mut nodes: HashMap<Rc<Path>, Node> = HashMap::new();
        let mut last_path = root_path.to_owned();
        let mut index = 0;

        while let Some(current_path) = stack.pop() {
            let current_depth = current_path.depth();
            if current_depth >= depth + root_depth {
                continue;
            }
            let children_will_be_added = depth + root_depth > 1 + current_depth;
            let children =
                if children_will_be_added && current_path.is_dir() && !current_path.is_symlink() {
                    Self::create_children(
                        &mut stack,
                        &current_path,
                        users,
                        show_hidden,
                        filter_kind,
                        sort_kind,
                    )
                } else {
                    None
                };
            let current_node = Node::new(&current_path, children, &last_path, index);
            Self::set_next_for_last(&mut nodes, &current_path, &last_path);
            last_path = current_path.clone();
            nodes.insert(current_path, current_node);
            index += 1;
        }
        Self::set_prev_for_root(&mut nodes, &root_path, &last_path);
        // Self::set_next_for_last(&mut nodes, &root_path, &last_path);
        nodes
    }

    #[inline]
    fn set_prev_for_root(nodes: &mut HashMap<Rc<Path>, Node>, root_path: &Path, last_path: &Path) {
        let Some(root_node) = nodes.get_mut(root_path) else {
            unreachable!("root_path should be in nodes");
        };
        root_node.prev = Rc::from(last_path);
        root_node.select();
    }

    #[inline]
    fn set_next_for_last(nodes: &mut HashMap<Rc<Path>, Node>, root_path: &Path, last_path: &Path) {
        if let Some(last_node) = nodes.get_mut(last_path) {
            last_node.next = Rc::from(root_path);
        };
    }

    #[inline]
    fn create_children(
        stack: &mut Vec<Rc<Path>>,
        current_path: &Path,
        users: &Users,
        show_hidden: bool,
        filter_kind: &FilterKind,
        sort_kind: SortKind,
    ) -> Option<Vec<Rc<Path>>> {
        if let Some(mut files) =
            files_collection(current_path, users, show_hidden, filter_kind, true)
        {
            sort_kind.sort(&mut files);
            let children = Self::make_children_and_stack_them(stack, &files);
            if !children.is_empty() {
                return Some(children);
            }
        }
        None
    }

    #[inline]
    fn make_children_and_stack_them(
        stack: &mut Vec<Rc<Path>>,
        files: &[FileInfo],
    ) -> Vec<Rc<Path>> {
        files
            .iter()
            .map(|fileinfo| fileinfo.path.clone())
            .map(|path| {
                stack.push(path.clone());
                path
            })
            .collect()
    }

    /// Root path of the tree.
    pub fn root_path(&self) -> &Path {
        self.root_path.borrow()
    }

    /// Selected path
    pub fn selected_path(&self) -> &Path {
        self.selected.borrow()
    }

    /// Selected node
    pub fn selected_node(&self) -> Option<&Node> {
        self.nodes.get(&self.selected)
    }

    /// The folder containing the selected node.
    /// Itself if selected is a directory.
    pub fn directory_of_selected(&self) -> Option<&Path> {
        if self.selected.is_dir() && !self.selected.is_symlink() {
            Some(self.selected.borrow())
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
        drop(next_path);
        self.increment_required_height()
    }

    fn find_next_path(&self) -> Rc<Path> {
        let mut current_path: Rc<Path> = self.selected.clone();
        loop {
            if let Some(current_node) = self.nodes.get(&current_path) {
                let next_path = &current_node.next;
                let Some(next_node) = self.nodes.get(next_path) else {
                    return self.root_path.clone();
                };
                if next_node.reachable {
                    return next_path.to_owned();
                } else {
                    current_path = next_path.clone();
                }
            }
        }
    }

    /// Select previous sibling or the parent
    fn select_prev(&mut self) {
        let must_increase = self.is_on_root();
        let previous_path = self.find_prev_path();
        self.select_path(&previous_path, false);
        drop(previous_path);
        if must_increase {
            self.set_required_height_to_max()
        } else {
            self.decrement_required_height()
        }
    }

    fn find_prev_path(&self) -> Rc<Path> {
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
        if Rc::from(dest_path) == self.selected {
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
        self.selected = Rc::from(dest_path);
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

    fn children_of_selected(&self) -> Vec<Rc<Path>> {
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
        self.select_path(&found_path, true);
    }

    fn deep_first_search(&self, pattern: &str) -> Option<Rc<Path>> {
        let mut stack = vec![self.root_path.clone()];
        let mut found = vec![];

        while let Some(path) = stack.pop() {
            if path_filename_contains(&path, pattern) {
                found.push(path.clone());
            }
            let Some(current_node) = self.nodes.get(&path) else {
                continue;
            };

            if current_node.have_children() {
                let Some(children) = &current_node.children else {
                    continue;
                };
                for leaf in children.iter() {
                    stack.push(leaf.to_owned());
                }
            }
        }
        self.pick_best_match(&found)
    }

    fn pick_best_match(&self, found: &[Rc<Path>]) -> Option<Rc<Path>> {
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
        let mut stack = vec![("".to_owned(), self.root_path.borrow())];
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
                ColorEffect::node(&fileinfo, node.selected()),
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

    #[inline]
    pub fn filenames_containing(&self, input_string: &str) -> Vec<String> {
        let to_filename: fn(&Rc<Path>) -> Option<&OsStr> = |path| path.file_name();
        let to_str: fn(&OsStr) -> Option<&str> = |filename| filename.to_str();
        self.nodes
            .keys()
            .filter_map(to_filename)
            .filter_map(to_str)
            .filter(|&p| p.contains(input_string))
            .map(|p| p.replace("▸ ", "").replace("▾ ", ""))
            .collect()
    }

    /// Vector of `Path` of nodes.
    pub fn paths(&self) -> Vec<&Path> {
        self.nodes.keys().map(|p| p.borrow()).collect()
    }

    /// True if any directory (not symlink to a directory)
    /// has been modified less than 10 seconds ago.
    #[inline]
    pub fn has_modified_dirs(&self) -> bool {
        self.nodes
            .keys()
            .filter(|path| path.is_dir() && !path.is_symlink())
            .any(|path| has_last_modification_happened_less_than(path, 10).unwrap_or(false))
    }

    #[inline]
    fn stack_children<'a>(
        stack: &mut Vec<(String, &'a Path)>,
        prefix: String,
        current_node: &'a Node,
    ) {
        let first_prefix = first_prefix(&prefix);
        let other_prefix = other_prefix(&prefix);

        let Some(children) = &current_node.children else {
            return;
        };
        let mut children = children.iter();
        let Some(first_leaf) = children.next() else {
            return;
        };
        stack.push((first_prefix, first_leaf));

        for leaf in children {
            stack.push((other_prefix.clone(), leaf));
        }
    }
}

#[inline]
fn first_prefix(prefix: &str) -> String {
    let mut prefix = prefix.to_string();
    prefix.push(' ');
    prefix = prefix.replace("└──", "  ");
    prefix = prefix.replace("├──", "│ ");
    prefix.push_str("└──");
    prefix
}

#[inline]
fn other_prefix(prefix: &str) -> String {
    let mut prefix = prefix.to_string();
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
#[inline]
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
