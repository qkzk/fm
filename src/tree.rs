use std::fs::read_dir;
use std::path::Path;
use std::rc::Rc;

use log::info;
use tuikit::attr::Attr;
use users::UsersCache;

use crate::config::Colors;
use crate::fileinfo::{fileinfo_attr, FileInfo, FileKind};
use crate::fm_error::FmResult;
use crate::status::Status;
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

    fn attr(&self, status: &Status, colors: &Colors) -> Attr {
        let mut attr = match self {
            Node::Directory(fileinfo) => fileinfo_attr(status, fileinfo, colors),
            Node::File(fileinfo) => fileinfo_attr(status, fileinfo, colors),
        };
        if match self {
            Self::Directory(fileinfo) => fileinfo.is_selected,
            Self::File(fileinfo) => fileinfo.is_selected,
        } {
            info!("node selected: {:?}", self);
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
        let fileinfo = FileInfo::from_path_with_name(path, filename, users_cache)?;
        let mut leaves = vec![];
        let node: Node;
        if let FileKind::Directory = fileinfo.file_kind {
            node = Node::Directory(fileinfo);
            if max_depth > 0 {
                for direntry in read_dir(path)?.filter_map(|d| d.ok()) {
                    leaves.push(Self::from_path(
                        &direntry.path(),
                        max_depth - 1,
                        users_cache,
                    )?)
                }
            }
        } else {
            node = Node::File(fileinfo);
        }
        Ok(Self { node, leaves })
    }

    pub fn empty(path: &Path, users_cache: &Rc<UsersCache>) -> FmResult<Self> {
        let filename = filename_from_path(path)?;
        let fileinfo = FileInfo::from_path_with_name(path, filename, users_cache)?;
        let node = Node::Directory(fileinfo);
        Ok(Self {
            node,
            leaves: vec![],
        })
    }

    pub fn select_root(&mut self) {
        self.node.select()
    }

    pub fn unselect_root(&mut self) {
        self.node.unselect()
    }

    pub fn select_node(&mut self) {}

    pub fn unselect_node(&mut self) {}

    /// Depth first traversal of the tree.
    /// We navigate into the tree and format every element into a pair :
    /// - a prefix, wich is a string made of glyphs displaying the tree,
    /// - a colored string to be colored relatively to the file type.
    /// Since we use the same colors everywhere, it's
    pub fn into_navigable_content(
        &self,
        status: &Status,
        colors: &Colors,
    ) -> Vec<(String, ColoredString)> {
        let mut stack = vec![];
        stack.push(("".to_owned(), self));
        let mut content = vec![];
        let mut current_node: Node;

        while !stack.is_empty() {
            if let Some((prefix, current)) = stack.pop() {
                current_node = current.node.clone();

                content.push((
                    prefix.to_owned(),
                    ColoredString::new(current_node.filename(), current_node.attr(status, colors)),
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
