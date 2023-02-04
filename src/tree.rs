use std::path::Path;

use tuikit::attr::Attr;
use users::UsersCache;

use crate::config::Colors;
use crate::fileinfo::{fileinfo_attr, files_collection, FileInfo, FileKind};
use crate::filter::FilterKind;
use crate::fm_error::FmResult;
use crate::sort::SortKind;
use crate::utils::filename_from_path;

/// Holds a string and its display attributes.
#[derive(Clone, Debug)]
pub struct ColoredString {
    /// A text to be printed. In most case, it should be a filename.
    pub text: String,
    /// A tuikit::attr::Attr (fg, bg, effect) to enhance the text.
    pub attr: Attr,
    /// The complete path of this string.
    pub path: std::path::PathBuf,
}

impl ColoredString {
    fn new(text: String, attr: Attr, path: std::path::PathBuf) -> Self {
        Self { text, attr, path }
    }

    fn from_node(current_node: &Node, colors: &Colors) -> Self {
        let text = if current_node.is_dir {
            if current_node.folded {
                format!("▸ {}", &current_node.fileinfo.filename)
            } else {
                format!("▾ {}", &current_node.fileinfo.filename)
            }
        } else {
            current_node.filename()
        };
        Self::new(text, current_node.attr(colors), current_node.filepath())
    }
}

/// An element in a tree.
/// Can be a directory or a file (other kind of file).
/// Both hold a fileinfo
#[derive(Clone, Debug)]
pub struct Node {
    pub fileinfo: FileInfo,
    pub position: Vec<usize>,
    pub folded: bool,
    pub is_dir: bool,
}

impl Node {
    /// Returns a copy of the filename.
    pub fn filename(&self) -> String {
        self.fileinfo.filename.to_owned()
    }

    /// Returns a copy of the filepath.
    pub fn filepath(&self) -> std::path::PathBuf {
        self.fileinfo.path.to_owned()
    }

    fn attr(&self, colors: &Colors) -> Attr {
        let mut attr = fileinfo_attr(&self.fileinfo, colors);
        if self.fileinfo.is_selected {
            attr.effect |= tuikit::attr::Effect::REVERSE
        };

        attr
    }

    fn select(&mut self) {
        self.fileinfo.select()
    }

    fn unselect(&mut self) {
        self.fileinfo.unselect()
    }

    /// Toggle the fold status of a node.
    pub fn toggle_fold(&mut self) {
        self.folded = !self.folded;
    }

    fn from_fileinfo(fileinfo: FileInfo, parent_position: Vec<usize>) -> Self {
        Self {
            is_dir: matches!(fileinfo.file_kind, FileKind::Directory),
            fileinfo,
            position: parent_position,
            folded: false,
        }
    }
}

/// Holds a recursive view of a directory.
/// Creation can be long as is explores every subfolder to a certain depth.
/// Parsing into a vector of "prefix" (String) and `ColoredString` is a depthfirst search
/// and it can be long too.
#[derive(Clone, Debug)]
pub struct Tree {
    pub node: Node,
    pub leaves: Vec<Tree>,
    pub position: Vec<usize>,
    pub current_node: Node,
    sort_kind: SortKind,
}

impl Tree {
    /// The max depth when exploring a tree.
    /// ATM it's a constant, in future versions it may change
    /// It may be better to stop the recursion when too much file
    /// are present and the exploration is slow.
    pub const MAX_DEPTH: usize = 7;

    /// Recursively explore every subfolder to a certain depth.
    /// We start from `path` and add this node first.
    /// Then, for every subfolder, we start again.
    /// Files in path are added as simple nodes.
    /// Both (subfolder and files) ends in a collections of leaves.
    pub fn from_path(
        path: &Path,
        max_depth: usize,
        users_cache: &UsersCache,
        filter_kind: &FilterKind,
        show_hidden: bool,
        parent_position: Vec<usize>,
    ) -> FmResult<Self> {
        Self::create_tree_from_fileinfo(
            FileInfo::from_path_with_name(path, filename_from_path(path)?, users_cache)?,
            max_depth,
            users_cache,
            filter_kind,
            show_hidden,
            parent_position,
        )
    }

    /// Clear every vector attributes of the tree.
    /// It's used to free some unused memory.
    pub fn clear(&mut self) {
        self.leaves = vec![];
        self.position = vec![];
    }

    /// A reference to the holded node fileinfo.
    pub fn file(&self) -> &FileInfo {
        &self.node.fileinfo
    }

    fn create_tree_from_fileinfo(
        fileinfo: FileInfo,
        max_depth: usize,
        users_cache: &UsersCache,
        filter_kind: &FilterKind,
        display_hidden: bool,
        parent_position: Vec<usize>,
    ) -> FmResult<Self> {
        let sort_kind = SortKind::tree_default();
        let leaves = Self::make_leaves(
            &fileinfo,
            max_depth,
            users_cache,
            display_hidden,
            filter_kind,
            &sort_kind,
            parent_position.clone(),
        )?;
        let node = Node::from_fileinfo(fileinfo, parent_position);
        let position = vec![0];
        let current_node = node.clone();
        Ok(Self {
            node,
            leaves,
            position,
            current_node,
            sort_kind,
        })
    }

    fn make_leaves(
        fileinfo: &FileInfo,
        max_depth: usize,
        users_cache: &UsersCache,
        display_hidden: bool,
        filter_kind: &FilterKind,
        sort_kind: &SortKind,
        parent_position: Vec<usize>,
    ) -> FmResult<Vec<Tree>> {
        if max_depth == 0 {
            return Ok(vec![]);
        }
        let FileKind::Directory = fileinfo.file_kind else { return Ok(vec![]) };
        let Some(mut files) =
                files_collection(fileinfo, users_cache, display_hidden, filter_kind)
            else { return Ok(vec![]) };
        sort_kind.sort(&mut files);
        let leaves = files
            .iter()
            .enumerate()
            .map(|(index, fileinfo)| {
                let mut position = parent_position.clone();
                position.push(files.len() - index - 1);
                Self::create_tree_from_fileinfo(
                    fileinfo.to_owned(),
                    max_depth - 1,
                    users_cache,
                    filter_kind,
                    display_hidden,
                    position,
                )
                .unwrap()
            })
            .collect();

        Ok(leaves)
    }

    /// Sort the leaves with current sort kind.
    pub fn sort(&mut self) {
        let sort_kind = self.sort_kind.clone();
        self.sort_tree_by_kind(&sort_kind);
    }

    fn sort_tree_by_kind(&mut self, sort_kind: &SortKind) {
        sort_kind.sort_tree(&mut self.leaves);
        for tree in self.leaves.iter_mut() {
            tree.sort_tree_by_kind(sort_kind);
        }
    }

    /// Creates an empty tree. Used when the user changes the CWD and hasn't displayed
    /// a tree yet.
    pub fn empty(path: &Path, users_cache: &UsersCache) -> FmResult<Self> {
        let filename = filename_from_path(path)?;
        let fileinfo = FileInfo::from_path_with_name(path, filename, users_cache)?;
        let node = Node {
            fileinfo,
            position: vec![0],
            folded: false,
            is_dir: false,
        };
        let leaves = vec![];
        let position = vec![0];
        let current_node = node.clone();
        let sort_kind = SortKind::tree_default();
        Ok(Self {
            node,
            leaves,
            position,
            current_node,
            sort_kind,
        })
    }

    pub fn update_sort_from_char(&mut self, c: char) {
        self.sort_kind.update_from_char(c)
    }

    /// Select the root node of the tree.
    pub fn select_root(&mut self) {
        self.node.select();
        self.position = vec![0]
    }

    /// Unselect every node in the tree.
    pub fn unselect_children(&mut self) {
        self.node.unselect();
        for tree in self.leaves.iter_mut() {
            tree.unselect_children()
        }
    }

    /// Fold every node in the tree.
    pub fn fold_children(&mut self) {
        self.node.folded = true;
        for tree in self.leaves.iter_mut() {
            tree.fold_children()
        }
    }

    /// Unfold every node in the tree.
    pub fn unfold_children(&mut self) {
        self.node.folded = false;
        for tree in self.leaves.iter_mut() {
            tree.unfold_children()
        }
    }

    /// Select the next "brother/sister" of a node.
    /// Sibling have the same parents (ie. are in the same directory).
    /// Since the position may be wrong (aka the current node is already the last child of
    /// it's parent) we have to adjust the postion afterwards.
    pub fn select_next_sibling(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0]
        } else {
            let len = self.position.len();
            self.position[len - 1] += 1;
            let (depth, last_cord, node) = self.select_from_position()?;
            self.fix_position(depth, last_cord);
            self.current_node = node;
        }
        Ok(())
    }

    /// Select the previous "brother/sister" of a node.
    /// Sibling have the same parents (ie. are in the same directory).
    /// Since the position may be wrong (aka the current node is already the first child of
    /// it's parent) we have to adjust the postion afterwards.
    pub fn select_prev_sibling(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0]
        } else {
            let len = self.position.len();
            if self.position[len - 1] > 0 {
                self.position[len - 1] -= 1;
            } else {
                self.select_parent()?
            }
            let (depth, last_cord, node) = self.select_from_position()?;
            self.fix_position(depth, last_cord);
            self.current_node = node;
        }
        Ok(())
    }

    fn fix_position(&mut self, depth: usize, last_cord: usize) {
        self.position.truncate(depth + 1);
        self.position[depth] = last_cord;
    }

    /// Select the first child of a current node.
    /// Does nothing if the node has no child.
    pub fn select_first_child(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0]
        }
        self.position.push(0);
        let (depth, last_cord, node) = self.select_from_position()?;
        self.fix_position(depth, last_cord);
        self.current_node = node;
        Ok(())
    }

    /// Move to the parent of current node.
    /// If the parent is the root node, it will do nothing.
    pub fn select_parent(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0];
        } else {
            self.position.pop();
            if self.position.is_empty() {
                self.position.push(0)
            }
            let (depth, last_cord, node) = self.select_from_position()?;
            self.fix_position(depth, last_cord);
            self.current_node = node
        }
        Ok(())
    }

    /// Move to the last leaf (bottom line on screen).
    /// We use a simple trick since we can't know how much node there is
    /// at every step.
    /// We first create a position with max value (usize::MAX) and max size (Self::MAX_DEPTH).
    /// Then we select this node and adjust the position.
    pub fn go_to_bottom_leaf(&mut self) -> FmResult<()> {
        self.position = vec![usize::MAX; Self::MAX_DEPTH];
        let (depth, last_cord, node) = self.select_from_position()?;
        self.fix_position(depth, last_cord);
        self.current_node = node;
        Ok(())
    }

    /// Select the node at a given position.
    /// Returns the reached depth, the last index and a copy of the node itself.
    pub fn select_from_position(&mut self) -> FmResult<(usize, usize, Node)> {
        let (tree, reached_depth, last_cord) = self.explore_position(true);
        tree.node.select();
        Ok((reached_depth, last_cord, tree.node.clone()))
    }

    /// Depth first traversal of the tree.
    /// We navigate into the tree and format every element into a pair :
    /// - a prefix, wich is a string made of glyphs displaying the tree,
    /// - a colored string to be colored relatively to the file type.
    /// Since we use the same colors everywhere, it's
    pub fn into_navigable_content(
        &mut self,
        colors: &Colors,
    ) -> (usize, Vec<(String, ColoredString)>) {
        let mut stack = vec![("".to_owned(), self)];
        let mut content = vec![];
        let mut selected_index = 0;

        while let Some((prefix, current)) = stack.pop() {
            if current.node.fileinfo.is_selected {
                selected_index = content.len();
            }

            content.push((
                prefix.to_owned(),
                ColoredString::from_node(&current.node, colors),
            ));

            if !current.node.folded {
                let first_prefix = first_prefix(prefix.clone());
                let other_prefix = other_prefix(prefix);

                let mut leaves = current.leaves.iter_mut();
                let Some(first_leaf) = leaves.next() else { continue; };
                stack.push((first_prefix.clone(), first_leaf));

                for leaf in leaves {
                    stack.push((other_prefix.clone(), leaf));
                }
            }
        }
        (selected_index, content)
    }

    /// Select the first node matching a key.
    /// We use a breath first search algorithm to ensure we select the less deep one.
    pub fn select_first_match(&mut self, key: &str) -> Option<Vec<usize>> {
        if self.node.fileinfo.filename.contains(key) {
            return Some(self.node.position.clone());
        }

        for tree in self.leaves.iter_mut().rev() {
            let Some(position) = tree.select_first_match(key) else { continue };
            return Some(position);
        }

        None
    }

    // TODO! refactor to return the new position vector and use it.
    /// Recursively explore the tree while only selecting the
    /// node from the position.
    /// Returns the reached tree, the reached depth and the last index.
    /// It may be used to fix the position.
    /// position is a vector of node indexes. At each step, we select the
    /// existing node.
    /// If `unfold` is set to true, it will unfold the trees as it traverses
    /// them.
    /// Since this method is used to fold every node, this parameter is required.
    pub fn explore_position(&mut self, unfold: bool) -> (&mut Tree, usize, usize) {
        let mut tree = self;
        let pos = tree.position.clone();
        let mut last_cord = 0;
        let mut reached_depth = 0;

        for (depth, &coord) in pos.iter().skip(1).enumerate() {
            if unfold {
                tree.node.folded = false;
            }
            last_cord = coord;
            if depth > pos.len() || tree.leaves.is_empty() {
                break;
            }
            if coord >= tree.leaves.len() {
                last_cord = tree.leaves.len() - 1;
            }
            let len = tree.leaves.len();
            tree = &mut tree.leaves[len - 1 - last_cord];
            reached_depth += 1;
        }
        (tree, reached_depth, last_cord)
    }

    pub fn position_from_index(&self, index: usize) -> Vec<usize> {
        let mut stack = vec![];
        stack.push(self);

        let mut visited = self;
        let mut counter = 0;
        while let Some(current) = stack.pop() {
            counter += 1;
            visited = current;
            if counter == index {
                break;
            }
            if !current.node.folded {
                for leaf in current.leaves.iter() {
                    stack.push(leaf);
                }
            }
        }

        visited.node.position.clone()
    }
}

fn first_prefix(mut prefix: String) -> String {
    prefix.push(' ');
    prefix = prefix.replace("└──", "   ");
    prefix = prefix.replace("├──", "│  ");
    prefix.push_str("└──");
    prefix
}

fn other_prefix(mut prefix: String) -> String {
    prefix.push(' ');
    prefix = prefix.replace("└──", "   ");
    prefix = prefix.replace("├──", "│  ");
    prefix.push_str("├──");
    prefix
}
