use std::borrow::Borrow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::rc::Rc;

use anyhow::Result;

use crate::common::filename_from_path;
use crate::common::has_last_modification_happened_less_than;
use crate::modes::files_collection;
use crate::modes::FilterKind;
use crate::modes::SortKind;
use crate::modes::Users;
use crate::modes::{ColorEffect, FileInfo};

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
            To::Path(path) => self.select_path(path),
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
    displayable_lines: TreeLines,
}

impl Default for Tree {
    fn default() -> Self {
        Self {
            root_path: Rc::from(Path::new("")),
            selected: Rc::from(Path::new("")),
            nodes: HashMap::new(),
            displayable_lines: TreeLines::default(),
        }
    }
}

impl Tree {
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

        let content = Self::make_displayable(users, &root_path, &nodes);

        Self {
            selected: root_path.clone(),
            root_path,
            nodes,
            displayable_lines: content,
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
        self.select_path(&next_path);
        drop(next_path);
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
        let previous_path = self.find_prev_path();
        self.select_path(&previous_path);
        drop(previous_path);
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

    fn select_path(&mut self, dest_path: &Path) {
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

    /// Select the first node whose filename match a pattern.
    /// If the selected file match, the next match will be selected.
    pub fn search_first_match(&mut self, pattern: &str) {
        let Some(found_path) = self.deep_first_search(pattern) else {
            return;
        };
        self.select_path(&found_path);
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
    fn make_displayable(
        users: &Users,
        root_path: &Path,
        nodes: &HashMap<Rc<Path>, Node>,
    ) -> TreeLines {
        let mut stack = vec![("".to_owned(), root_path.borrow())];
        let mut lines = vec![];
        let mut index = 0;

        while let Some((prefix, path)) = stack.pop() {
            let Some(node) = nodes.get(path) else {
                continue;
            };

            if node.selected {
                index = lines.len();
            }

            let Ok(fileinfo) = FileInfo::new(path, users) else {
                continue;
            };

            lines.push(TreeLineBuilder::new(&fileinfo, &prefix, node, path));

            if node.have_children() {
                Self::stack_children(&mut stack, prefix, node);
            }
        }
        TreeLines::new(lines, index)
    }

    fn remake_displayable(&mut self, users: &Users) {
        self.displayable_lines = Self::make_displayable(users, &self.root_path, &self.nodes);
    }

    pub fn displayable(&self) -> &TreeLines {
        &self.displayable_lines
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

fn path_filename_contains(path: &Path, pattern: &str) -> bool {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .contains(pattern)
}

/// A vector of displayable lines used to draw a tree content.
/// We use the index to follow the user movements in the tree.
#[derive(Clone, Debug, Default)]
pub struct TreeLines {
    pub content: Vec<TreeLineBuilder>,
    index: usize,
}

impl TreeLines {
    fn new(content: Vec<TreeLineBuilder>, index: usize) -> Self {
        Self { content, index }
    }

    /// Index of the currently selected file.
    pub fn index(&self) -> usize {
        self.index
    }

    /// A reference to the displayable lines.
    pub fn lines(&self) -> &Vec<TreeLineBuilder> {
        &self.content
    }

    fn find_by_path(&self, path: &Path) -> Option<usize> {
        self.content
            .iter()
            .position(|tlm| <Rc<std::path::Path> as Borrow<Path>>::borrow(&tlm.path) == path)
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
}

/// Holds a few references used to display a tree line
/// Only the metadata info is hold.
#[derive(Clone, Debug)]
pub struct TreeLineBuilder {
    folded: bool,
    prefix: std::rc::Rc<str>,
    path: std::rc::Rc<Path>,
    color_effect: ColorEffect,
    metadata: String,
}

impl TreeLineBuilder {
    /// Uses references to fileinfo, prefix, node & path to create an instance.
    fn new(fileinfo: &FileInfo, prefix: &str, node: &Node, path: &Path) -> Self {
        let color_effect = ColorEffect::node(fileinfo, node.selected());
        let prefix = Rc::from(prefix);
        let path = Rc::from(path);
        let metadata = fileinfo
            .format_no_filename()
            .unwrap_or_else(|_| "?".repeat(19));
        let folded = node.folded;

        Self {
            folded,
            prefix,
            path,
            color_effect,
            metadata,
        }
    }

    /// Formated filename
    pub fn filename(&self) -> String {
        filename_format(&self.path, self.folded)
    }

    /// `tuikit::attr::Attr` of the line
    pub fn attr(&self) -> tuikit::attr::Attr {
        self.color_effect.attr()
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
        self.color_effect.effect = tuikit::attr::Effect::empty();
    }

    /// Change the current effect to `REVERSE`, displaying
    /// the file as selected.
    pub fn select(&mut self) {
        self.color_effect.effect = tuikit::attr::Effect::REVERSE;
    }
}
