use std::fs::read_dir;
use std::path::Path;
use std::rc::Rc;

use tuikit::attr::Attr;
use users::UsersCache;

use crate::config::Colors;
use crate::fileinfo::{fileinfo_attr, FileInfo, FileKind};
use crate::fm_error::FmResult;
use crate::utils::filename_from_path;

/// Holds a string and its display attributes.
#[derive(Clone, Debug)]
pub struct ColoredString {
    /// A text to be printed. In most case, it should be a filename.
    pub text: String,
    /// A tuikit::attr::Attr (fg, bg, effect) to enhance the text.
    pub attr: Attr,
}

impl ColoredString {
    fn new(text: String, attr: Attr) -> Self {
        Self { text, attr }
    }
}

/// An element in a tree.
/// Can be a directory or a file (other kind of file).
/// Both hold a fileinfo
#[derive(Clone, Debug)]
pub enum Node {
    Directory(FileInfo),
    File(FileInfo),
}

impl Node {
    fn filename(&self) -> String {
        match self {
            Node::Directory(fileinfo) => fileinfo.filename.to_owned(),
            Node::File(fileinfo) => fileinfo.filename.to_owned(),
        }
    }

    fn filepath(&self) -> std::path::PathBuf {
        match self {
            Node::Directory(fileinfo) => fileinfo.path.to_owned(),
            Node::File(fileinfo) => fileinfo.path.to_owned(),
        }
    }

    fn attr(&self, colors: &Colors) -> Attr {
        let mut attr = match self {
            Node::Directory(fileinfo) => fileinfo_attr(fileinfo, colors),
            Node::File(fileinfo) => fileinfo_attr(fileinfo, colors),
        };
        if match self {
            Self::Directory(fileinfo) => fileinfo.is_selected,
            Self::File(fileinfo) => fileinfo.is_selected,
        } {
            attr.effect |= tuikit::attr::Effect::REVERSE
        };

        attr
    }

    pub fn select(&mut self) {
        match self {
            Self::Directory(fileinfo) => fileinfo.select(),
            Self::File(fileinfo) => fileinfo.select(),
        }
    }

    pub fn unselect(&mut self) {
        match self {
            Self::Directory(fileinfo) => fileinfo.unselect(),
            Self::File(fileinfo) => fileinfo.unselect(),
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
    pub current_path: std::path::PathBuf,
}

impl Tree {
    /// Recursively explore every subfolder to a certain depth.
    /// We start from `path` and add this node first.
    /// Then, for every subfolder, we start again.
    /// Files in path are added as simple nodes.
    /// Both (subfolder and files) ends in a collections of leaves.
    ///
    /// TODO!
    /// The `users_cache` parameter isn't really used atm, we need it
    /// to create `FileInfo` objects, which use this structure to determine
    /// the FileKind (socket, device, char, block, normal file, directory etc.)
    /// The FileKind is used to determine the color later on.
    ///
    /// TODO!
    /// make it really navigable
    /// left : -> parent
    /// right: -> step into ?
    /// down: next sibling
    /// up: previous sibling
    /// TODO!
    /// make it foldable
    pub fn from_path(
        path: &Path,
        max_depth: usize,
        users_cache: &Rc<UsersCache>,
    ) -> FmResult<Self> {
        let filename = filename_from_path(path)?;
        match FileInfo::from_path_with_name(path, filename, users_cache) {
            Ok(fileinfo) => {
                let mut leaves = vec![];
                let node: Node;
                if let FileKind::Directory = fileinfo.file_kind {
                    node = Node::Directory(fileinfo);
                    if max_depth > 0 {
                        for direntry in read_dir(path)?.filter_map(|d| d.ok()) {
                            if let Ok(leaf) =
                                Self::from_path(&direntry.path(), max_depth - 1, users_cache)
                            {
                                leaves.push(leaf)
                            }
                        }
                    }
                } else {
                    node = Node::File(fileinfo);
                }
                let position = vec![0];
                let selected = node.filepath();
                Ok(Self {
                    node,
                    leaves,
                    position,
                    current_path: selected,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub fn empty(path: &Path, users_cache: &Rc<UsersCache>) -> FmResult<Self> {
        let filename = filename_from_path(path)?;
        let fileinfo = FileInfo::from_path_with_name(path, filename, users_cache)?;
        let node = Node::Directory(fileinfo);
        let leaves = vec![];
        let position = vec![0];
        let selected = node.filepath();
        Ok(Self {
            node,
            leaves,
            position,
            current_path: selected,
        })
    }

    pub fn select_root(&mut self) {
        self.node.select();
        self.position = vec![0]
    }

    pub fn unselect_children(&mut self) {
        self.node.unselect();
        for tree in self.leaves.iter_mut() {
            tree.unselect_children()
        }
    }

    pub fn select_next_sibling(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0]
        } else {
            let len = self.position.len();
            self.position[len - 1] += 1;
            let (depth, last_cord, filepath) = self.select_from_position()?;
            self.fix_position(depth, last_cord);
            self.current_path = filepath;
        }
        Ok(())
    }

    pub fn select_prev_sibling(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0]
        } else {
            let len = self.position.len();
            if self.position[len - 1] > 0 {
                self.position[len - 1] -= 1;
            }
            let (depth, last_cord, filepath) = self.select_from_position()?;
            self.fix_position(depth, last_cord);
            self.current_path = filepath;
        }
        Ok(())
    }

    fn fix_position(&mut self, depth: usize, last_cord: usize) {
        self.position.truncate(depth + 1);
        self.position[depth] = last_cord;
    }

    pub fn select_first_child(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0]
        }
        self.position.push(0);
        let (depth, last_cord, filepath) = self.select_from_position()?;
        self.fix_position(depth, last_cord);
        self.current_path = filepath;
        Ok(())
    }

    pub fn select_parent(&mut self) -> FmResult<()> {
        if self.position.is_empty() {
            self.position = vec![0];
        } else {
            self.position.pop();
            if self.position.is_empty() {
                self.position.push(0)
            }
            let (depth, last_cord, filepath) = self.select_from_position()?;
            self.fix_position(depth, last_cord);
            self.current_path = filepath
        }
        Ok(())
    }

    fn select_from_position(&mut self) -> FmResult<(usize, usize, std::path::PathBuf)> {
        let pos = self.position.clone();
        let mut tree = self;
        let mut reached_depth = 0;
        let mut last_cord = 0;
        for (depth, &coord) in pos.iter().skip(1).enumerate() {
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
        tree.node.select();
        Ok((reached_depth, last_cord, tree.node.filepath()))
    }

    /// Depth first traversal of the tree.
    /// We navigate into the tree and format every element into a pair :
    /// - a prefix, wich is a string made of glyphs displaying the tree,
    /// - a colored string to be colored relatively to the file type.
    /// Since we use the same colors everywhere, it's
    pub fn into_navigable_content(&self, colors: &Colors) -> Vec<(String, ColoredString)> {
        let mut stack = vec![];
        stack.push(("".to_owned(), self));
        let mut content = vec![];
        let mut current_node: Node;

        while !stack.is_empty() {
            if let Some((prefix, current)) = stack.pop() {
                current_node = current.node.clone();

                content.push((
                    prefix.to_owned(),
                    ColoredString::new(current_node.filename(), current_node.attr(colors)),
                ));

                let first_prefix = first_prefix(prefix.clone());
                let other_prefix = other_prefix(prefix);

                for (index, leaf) in current.leaves.iter().enumerate() {
                    if index == 0 {
                        stack.push((first_prefix.clone(), leaf));
                    } else {
                        stack.push((other_prefix.clone(), leaf))
                    }
                }
            }
        }
        content
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
