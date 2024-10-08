use std::borrow::Borrow;
use std::cmp::min;
use std::collections::HashMap;
use std::iter::{Chain, Enumerate, Skip, Take};
use std::path::Path;
use std::slice::Iter;
use std::sync::Arc;

use anyhow::{Context, Result};
use tuikit::attr::Attr;

use crate::common::{filename_from_path, has_last_modification_happened_less_than};
use crate::modes::{
    files_collection, ContentWindow, FileInfo, FilterKind, Flagged, SortKind, ToPath, Users,
};

/// Holds a string, its display attributes and the associated pathbuf.
#[derive(Clone, Debug)]
pub struct ColoredString {
    /// A text to be printed. In most case, it should be a filename.
    pub text: String,
    /// A [`tuikit::attr::Attr`] used to enhance the text.
    pub attr: Attr,
    /// The complete path of this string.
    pub path: Arc<Path>,
}

impl ColoredString {
    /// Creates a new colored string.
    pub fn new(text: String, attr: Attr, path: Arc<Path>) -> Self {
        Self { text, attr, path }
    }
}

/// An element of a tree.
/// It's a file/directory, some optional children.
/// A Node knows if it's folded or selected.
#[derive(Debug, Clone)]
pub struct Node {
    path: Arc<Path>,
    prev: Arc<Path>,
    next: Arc<Path>,
    index: usize,
    children: Option<Vec<Arc<Path>>>,
    folded: bool,
    selected: bool,
    reachable: bool,
}

impl Node {
    /// Creates a new Node from a path and its children.
    /// By default it's not selected nor folded.
    fn new(path: &Path, children: Option<Vec<Arc<Path>>>, prev: &Path, index: usize) -> Self {
        Self {
            path: Arc::from(path),
            prev: Arc::from(prev),
            next: Arc::from(Path::new("")),
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

    /// Path of this node
    pub fn path(&self) -> &Path {
        &self.path
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
    NextSibling,
    PreviousSibling,
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
            To::NextSibling => self.select_next_sibling(),
            To::PreviousSibling => self.select_previous_sibling(),
            To::Path(path) => self.select_path(path),
        }
    }
}

/// Trait allowing to measure the depth of something.
trait Depth {
    fn depth(&self) -> usize;
}

impl Depth for Arc<Path> {
    /// Measure the number of components of a [`std::path::Path`].
    /// For absolute paths, it's the number of folders plus one for / and one for the file itself.
    #[inline]
    fn depth(&self) -> usize {
        self.components().collect::<Vec<_>>().len()
    }
}

pub struct TreeBuilder<'a> {
    root_path: Arc<Path>,
    users: &'a Users,
    filter_kind: &'a FilterKind,
    max_depth: usize,
    show_hidden: bool,
    sort_kind: SortKind,
}

impl<'a> TreeBuilder<'a> {
    const DEFAULT_DEPTH: usize = 5;
    const DEFAULT_FILTER: FilterKind = FilterKind::All;
    const DEFAULT_HIDDEN: bool = false;
    const DEFAULT_SORT: SortKind = SortKind::tree_default();

    pub fn new(root_path: Arc<Path>, users: &'a Users) -> Self {
        let filter_kind = &Self::DEFAULT_FILTER;
        let max_depth = Self::DEFAULT_DEPTH;
        let show_hidden = Self::DEFAULT_HIDDEN;
        let sort_kind = Self::DEFAULT_SORT;
        Self {
            root_path,
            users,
            filter_kind,
            max_depth,
            show_hidden,
            sort_kind,
        }
    }

    pub fn with_filter_kind(mut self, filter_kind: &'a FilterKind) -> Self {
        self.filter_kind = filter_kind;
        self
    }

    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    pub fn with_hidden(mut self, show_hidden: bool) -> Self {
        self.show_hidden = show_hidden;
        self
    }

    pub fn with_sort_kind(mut self, sort_kind: SortKind) -> Self {
        self.sort_kind = sort_kind;
        self
    }

    pub fn build(self) -> Tree {
        let nodes = NodesBuilder::new(
            &self.root_path,
            self.max_depth,
            self.sort_kind,
            self.users,
            self.show_hidden,
            self.filter_kind,
        )
        .build();
        let displayable_lines = TreeLinesBuilder::new(&nodes, &self.root_path, self.users).build();

        Tree {
            selected: self.root_path.clone(),
            root_path: self.root_path,
            nodes,
            displayable_lines,
        }
    }
}

pub struct NodesBuilder<'a> {
    root_path: &'a Arc<Path>,
    max_depth: usize,
    sort_kind: SortKind,
    users: &'a Users,
    show_hidden: bool,
    filter_kind: &'a FilterKind,
    root_depth: usize,
}

impl<'a> NodesBuilder<'a> {
    fn new(
        root_path: &'a Arc<Path>,
        max_depth: usize,
        sort_kind: SortKind,
        users: &'a Users,
        show_hidden: bool,
        filter_kind: &'a FilterKind,
    ) -> Self {
        let root_depth = root_path.depth();
        Self {
            root_path,
            max_depth,
            sort_kind,
            users,
            show_hidden,
            filter_kind,
            root_depth,
        }
    }

    #[inline]
    fn build(self) -> HashMap<Arc<Path>, Node> {
        let mut stack = vec![self.root_path.to_owned()];
        let mut nodes = HashMap::new();
        let mut last_path = self.root_path.to_owned();
        let mut index = 0;

        while let Some(current_path) = stack.pop() {
            let current_depth = current_path.depth();
            if self.current_is_too_deep(current_depth) {
                continue;
            }
            let children = if self.node_may_have_children(current_depth, &current_path) {
                self.create_children(&mut stack, &current_path)
            } else {
                None
            };
            let current_node = Node::new(&current_path, children, &last_path, index);
            self.set_next_for_last(&mut nodes, &current_path, &last_path);
            last_path = current_path.clone();
            nodes.insert(current_path.clone(), current_node);
            index += 1;
        }
        self.set_prev_for_root(&mut nodes, last_path);
        nodes
    }

    fn current_is_too_deep(&self, current_depth: usize) -> bool {
        current_depth >= self.max_depth + self.root_depth
    }

    fn node_may_have_children(&self, current_depth: usize, current_path: &Path) -> bool {
        self.is_not_too_deep_for_children(current_depth)
            && current_path.is_dir()
            && !current_path.is_symlink()
    }

    fn is_not_too_deep_for_children(&self, current_depth: usize) -> bool {
        self.root_depth + self.max_depth > 1 + current_depth
    }

    #[inline]
    fn set_prev_for_root(&self, nodes: &mut HashMap<Arc<Path>, Node>, last_path: Arc<Path>) {
        let Some(root_node) = nodes.get_mut(self.root_path) else {
            unreachable!("root_path should be in nodes");
        };
        root_node.prev = last_path;
        root_node.select();
    }

    #[inline]
    fn set_next_for_last(
        &self,
        nodes: &mut HashMap<Arc<Path>, Node>,
        current_path: &Path,
        last_path: &Path,
    ) {
        if let Some(last_node) = nodes.get_mut(last_path) {
            last_node.next = Arc::from(current_path);
        };
    }

    #[inline]
    fn create_children(
        &self,
        stack: &mut Vec<Arc<Path>>,
        current_path: &Path,
    ) -> Option<Vec<Arc<Path>>> {
        if let Some(mut files) = files_collection(
            current_path,
            self.users,
            self.show_hidden,
            self.filter_kind,
            true,
        ) {
            self.sort_kind.sort(&mut files);
            let children = Self::make_children_and_stack_them(stack, &files);
            if !children.is_empty() {
                return Some(children);
            }
        }
        None
    }

    #[inline]
    fn make_children_and_stack_them(
        stack: &mut Vec<Arc<Path>>,
        files: &[FileInfo],
    ) -> Vec<Arc<Path>> {
        files
            .iter()
            .map(|fileinfo| fileinfo.path.clone())
            .inspect(|path| stack.push(path.clone()))
            .collect()
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
fn filename_format(current_path: &Path, folded: bool) -> String {
    let filename = filename_from_path(current_path)
        .unwrap_or_default()
        .to_owned();

    if current_path.is_dir() && !current_path.is_symlink() {
        if folded {
            format!("▸ {}", filename)
        } else {
            format!("▾ {}", filename)
        }
    } else {
        filename
    }
}

struct TreeLinesBuilder<'a> {
    nodes: &'a HashMap<Arc<Path>, Node>,
    root_path: &'a Arc<Path>,
    users: &'a Users,
}

impl<'a> TreeLinesBuilder<'a> {
    fn new(
        nodes: &'a HashMap<Arc<Path>, Node>,
        root_path: &'a Arc<Path>,
        users: &'a Users,
    ) -> Self {
        Self {
            nodes,
            root_path,
            users,
        }
    }

    /// Create a displayable content from the tree.
    /// Returns 2 informations :
    /// - the index of the selected node into this content.
    ///      It's usefull to know where the user clicked
    /// - a vector of `TreeLineMaker` which holds every information
    ///     needed to display the tree.
    ///     We try to keep as much reference as possible and generate
    ///     the information lazyly, avoiding as much useless calcuations
    ///     as possible.
    ///     The metadata information (permissions, modified time etc.) must be
    ///     calculated immediatly, therefore for every node, since it requires
    ///     an access to the user list.
    ///     The prefix (straight lines displaying targets) must also be calcuated immediatly.
    ///     Name format is calculated on the fly.
    fn build(self) -> TreeLines {
        let mut stack = vec![("".to_owned(), self.root_path.clone())];
        let mut lines = vec![];
        let mut index = 0;

        while let Some((prefix, path)) = stack.pop() {
            let Some(node) = self.nodes.get(&path) else {
                continue;
            };

            if node.selected {
                index = lines.len();
            }

            let Ok(fileinfo) = FileInfo::new(&path, self.users) else {
                continue;
            };

            lines.push(TLine::new(&fileinfo, &prefix, node, &path));

            if node.have_children() {
                Self::stack_children(&mut stack, prefix, node);
            }
        }
        TreeLines::new(lines, index)
    }

    #[inline]
    fn stack_children(stack: &mut Vec<(String, Arc<Path>)>, prefix: String, current_node: &Node) {
        let first_prefix = first_prefix(&prefix);
        let other_prefix = other_prefix(&prefix);

        let Some(children) = &current_node.children else {
            return;
        };
        let mut children = children.iter();
        let Some(first_leaf) = children.next() else {
            return;
        };
        stack.push((first_prefix, first_leaf.clone()));

        for leaf in children {
            stack.push((other_prefix.clone(), leaf.clone()));
        }
    }
}

/// A vector of displayable lines used to draw a tree content.
/// We use the index to follow the user movements in the tree.
#[derive(Clone, Debug, Default)]
pub struct TreeLines {
    pub content: Vec<TLine>,
    index: usize,
}

impl TreeLines {
    fn new(content: Vec<TLine>, index: usize) -> Self {
        Self { content, index }
    }

    pub fn content(&self) -> &Vec<TLine> {
        &self.content
    }

    /// Index of the currently selected file.
    pub fn index(&self) -> usize {
        self.index
    }

    /// A reference to the displayable lines.
    pub fn lines(&self) -> &Vec<TLine> {
        &self.content
    }

    fn find_by_path(&self, path: &Path) -> Option<usize> {
        self.content
            .iter()
            .position(|tlm| <Arc<std::path::Path> as Borrow<Path>>::borrow(&tlm.path) == path)
    }

    fn unselect(&mut self) {
        if !self.content.is_empty() {
            self.content[self.index].unselect()
        }
    }

    fn select(&mut self, index: usize) {
        if !self.content.is_empty() {
            self.index = index;
            self.content[self.index].select()
        }
    }

    fn selected_is_last(&self) -> bool {
        self.index + 1 == self.content.len()
    }
}

/// Holds a few references used to display a tree line
/// Only the metadata info is hold.
#[derive(Clone, Debug)]
pub struct TLine {
    folded: bool,
    prefix: Arc<str>,
    pub path: Arc<Path>,
    pub attr: Attr,
    metadata: String,
}

impl TLine {
    /// Uses references to fileinfo, prefix, node & path to create an instance.
    fn new(fileinfo: &FileInfo, prefix: &str, node: &Node, path: &Path) -> Self {
        let mut attr = fileinfo.attr();
        // required for some edge cases when opening the tree while "." is the selected file
        if node.selected() {
            attr.effect |= tuikit::attr::Effect::REVERSE;
        }
        let prefix = Arc::from(prefix);
        let path = Arc::from(path);
        let metadata = fileinfo
            .format_no_filename()
            .unwrap_or_else(|_| "?".repeat(19));
        let folded = node.folded;

        Self {
            folded,
            prefix,
            path,
            attr,
            metadata,
        }
    }

    /// Formated filename
    pub fn filename(&self) -> String {
        filename_format(&self.path, self.folded)
    }

    /// Vertical bar displayed before the filename to show
    /// the adress of the file
    pub fn prefix(&self) -> &str {
        self.prefix.borrow()
    }

    /// Path of the file
    pub fn path(&self) -> &Path {
        self.path.borrow()
    }

    /// Metadata string representation
    /// permission, size, owner, groupe, modification date
    pub fn metadata(&self) -> &str {
        &self.metadata
    }

    /// Change the current effect to Empty, displaying
    /// the file as not selected
    pub fn unselect(&mut self) {
        self.attr.effect = tuikit::attr::Effect::empty();
    }

    /// Change the current effect to `REVERSE`, displaying
    /// the file as selected.
    pub fn select(&mut self) {
        self.attr.effect = tuikit::attr::Effect::REVERSE;
    }
}

impl ToPath for TLine {
    fn to_path(&self) -> &Path {
        &self.path
    }
}

/// A FileSystem tree of nodes.
/// Internally it's a wrapper around an `std::collections::HashMap<PathBuf, Node>`
/// It also holds informations about the required height of the tree.
#[derive(Debug, Clone)]
pub struct Tree {
    root_path: Arc<Path>,
    selected: Arc<Path>,
    nodes: HashMap<Arc<Path>, Node>,
    displayable_lines: TreeLines,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            root_path: Arc::from(Path::new("")),
            selected: Arc::from(Path::new("")),
            nodes: HashMap::new(),
            displayable_lines: TreeLines::default(),
        }
    }
}

impl Tree {
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

    pub fn display_len(&self) -> usize {
        self.displayable().lines().len()
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

    /// True if the node has a parent which is in `self.nodes` and is folded.
    /// This hacky and inefficient solution is necessary to know if we can select
    /// this node in `select_prev` `select_next`.
    /// It will construct all parent path from `/` to `node.path` except for the last one.
    fn node_has_parent_folded(&self, node: &Node) -> bool {
        let node_path = node.path();
        let mut current_path = std::path::PathBuf::from("/");
        for part in node_path.components() {
            current_path = current_path.join(part.as_os_str());
            if current_path == node_path {
                continue;
            }
            if let Some(current_node) = self.nodes.get(current_path.as_path()) {
                if current_node.folded {
                    return true;
                }
            }
        }
        false
    }

    /// Select next sibling or the next sibling of the parent
    fn select_next(&mut self) {
        let next_path = self.find_next_path();
        self.select_path(&next_path);
        drop(next_path);
    }

    fn find_next_path(&self) -> Arc<Path> {
        let mut current_path: Arc<Path> = self.selected.clone();
        loop {
            if let Some(current_node) = self.nodes.get(&current_path) {
                let next_path = &current_node.next;
                let Some(next_node) = self.nodes.get(next_path) else {
                    return self.root_path.clone();
                };
                if next_node.reachable && !self.node_has_parent_folded(next_node) {
                    return next_path.to_owned();
                }
                current_path = next_path.clone();
            }
        }
    }

    /// Select previous sibling or the parent
    fn select_prev(&mut self) {
        let previous_path = self.find_prev_path();
        self.select_path(&previous_path);
        drop(previous_path);
    }

    fn find_prev_path(&self) -> Arc<Path> {
        let mut current_path = self.selected.to_owned();
        loop {
            if let Some(current_node) = self.nodes.get(&current_path) {
                let prev_path = &current_node.prev;
                let Some(prev_node) = self.nodes.get(prev_path) else {
                    unreachable!("");
                };
                if prev_node.reachable && !self.node_has_parent_folded(prev_node) {
                    return prev_path.to_owned();
                }
                current_path = prev_path.to_owned();
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
        self.select_path(&root_path);
    }

    fn select_last(&mut self) {
        self.select_root();
        self.select_prev();
    }

    fn select_parent(&mut self) {
        if let Some(parent_path) = self.selected.parent() {
            self.select_path(parent_path.to_owned().as_path());
        }
    }

    fn find_siblings(&self) -> &Option<Vec<Arc<Path>>> {
        let Some(parent_path) = self.selected.parent() else {
            return &None;
        };
        let Some(parent_node) = self.nodes.get(parent_path) else {
            return &None;
        };
        &parent_node.children
    }

    fn select_next_sibling(&mut self) {
        let Some(children) = self.find_siblings() else {
            self.select_next();
            return;
        };
        let Some(curr_index) = children.iter().position(|p| p == &self.selected) else {
            return;
        };
        let next_index = if curr_index > 0 {
            curr_index - 1
        } else {
            children.len().checked_sub(1).unwrap_or_default()
        };
        let sibling_path = children[next_index].clone();
        self.select_path(&sibling_path);
    }

    fn select_previous_sibling(&mut self) {
        let Some(children) = self.find_siblings() else {
            self.select_prev();
            return;
        };
        let Some(curr_index) = children.iter().position(|p| p == &self.selected) else {
            return;
        };
        let next_index = (curr_index + 1) % children.len();
        let sibling_path = children[next_index].clone();
        self.select_path(&sibling_path);
    }

    fn select_path(&mut self, dest_path: &Path) {
        if Arc::from(dest_path) == self.selected {
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
        self.selected = Arc::from(dest_path);
        self.displayable_lines.unselect();
        if let Some(index) = self.displayable_lines.find_by_path(dest_path) {
            self.displayable_lines.select(index);
        }
    }

    /// Fold selected node
    pub fn toggle_fold(&mut self, users: &Users) {
        if let Some(node) = self.nodes.get_mut(&self.selected) {
            if node.folded {
                node.unfold();
                self.make_children_reachable()
            } else {
                node.fold();
                self.make_children_unreachable()
            }
        }
        self.remake_displayable(users);
    }

    fn children_of_selected(&self) -> Vec<Arc<Path>> {
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
    pub fn fold_all(&mut self, users: &Users) {
        for (_, node) in self.nodes.iter_mut() {
            node.fold()
        }
        self.select_root();
        self.remake_displayable(users);
    }

    /// Unfold all node from root to end
    pub fn unfold_all(&mut self, users: &Users) {
        for (_, node) in self.nodes.iter_mut() {
            node.unfold()
        }
        self.remake_displayable(users);
    }

    fn remake_displayable(&mut self, users: &Users) {
        self.displayable_lines = TreeLinesBuilder::new(&self.nodes, &self.root_path, users).build();
    }

    pub fn displayable(&self) -> &TreeLines {
        &self.displayable_lines
    }

    /// Vector of `Path` of nodes.
    pub fn paths(&self) -> Vec<&Path> {
        self.nodes.keys().map(|p| p.borrow()).collect()
    }

    pub fn flag_all(&self, flagged: &mut Flagged) {
        self.nodes
            .keys()
            .for_each(|p| flagged.push(p.to_path_buf()))
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

    pub fn selected_is_last(&self) -> bool {
        self.displayable_lines.selected_is_last()
    }

    pub fn path_from_index(&self, index: usize) -> Result<std::path::PathBuf> {
        let displayable = self.displayable();
        Ok(displayable
            .lines()
            .get(index)
            .context("tree: no selected file")?
            .path()
            .to_owned())
    }

    pub fn lines_enum_skip_take(
        &self,
        window: &ContentWindow,
    ) -> Take<Skip<Enumerate<Iter<TLine>>>> {
        let lines = self.displayable().lines();
        let length = lines.len();
        lines
            .iter()
            .enumerate()
            .skip(window.top)
            .take(min(length, window.bottom + 1))
    }

    /// Iterate over line from current index to bottom then from top to current inde.
    ///
    /// Useful when going to next match in search results
    pub fn iter_from_index_to_index(&self) -> Chain<Skip<Iter<TLine>>, Take<Iter<TLine>>> {
        let displayable = self.displayable();
        let index = displayable.index();
        let lines = displayable.lines();

        lines.iter().skip(index + 1).chain(lines.iter().take(index))
    }
}
