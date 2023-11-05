use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    content_window::ContentWindow,
    fileinfo::{files_collection, ColorEffect, FileInfo},
    filter::FilterKind,
    preview::ColoredTriplet,
    sort::SortKind,
    tree::ColoredString,
    users::Users,
    utils::filename_from_path,
};
#[derive(Debug, Clone)]
pub struct Node {
    pub path: PathBuf,
    pub children: Option<Vec<PathBuf>>,
    pub folded: bool,
    pub selected: bool,
}

impl Node {
    pub fn new(path: &Path, children: Option<Vec<PathBuf>>) -> Self {
        Self {
            path: path.to_owned(),
            children,
            folded: false,
            selected: false,
        }
    }

    pub fn fold(&mut self) {
        self.folded = true
    }

    pub fn unfold(&mut self) {
        self.folded = false
    }

    pub fn toggle_fold(&mut self) {
        self.folded = !self.folded
    }

    pub fn select(&mut self) {
        self.selected = true
    }

    pub fn unselect(&mut self) {
        self.selected = false
    }

    pub fn fileinfo(&self, users: &Users) -> Result<FileInfo> {
        FileInfo::new(&self.path, users)
    }

    pub fn color_effect(&self, users: &Users) -> Result<ColorEffect> {
        Ok(ColorEffect::new(&self.fileinfo(users)?))
    }

    pub fn set_children(&mut self, children: Option<Vec<PathBuf>>) {
        self.children = children
    }
}

#[derive(Debug, Clone)]
pub struct FileSystem {
    root_path: PathBuf,
    selected: PathBuf,
    nodes: HashMap<PathBuf, Node>,
    required_height: usize,
}

impl FileSystem {
    pub const REQUIRED_HEIGHT: usize = 80;

    pub fn new(
        root_path: PathBuf,
        depth: usize,
        sort_kind: SortKind,
        users: &Users,
        show_hidden: bool,
        filter_kind: &FilterKind,
    ) -> Self {
        // keep track of the depth
        let start_depth = root_path.components().collect::<Vec<_>>().len();
        let mut stack = vec![root_path.to_owned()];
        let mut nodes: HashMap<PathBuf, Node> = HashMap::new();

        while let Some(path) = stack.pop() {
            let reached_depth = path.components().collect::<Vec<_>>().len();
            if reached_depth >= depth + start_depth {
                continue;
            }
            let mut node = Node::new(&path, None);
            if let Ok(fileinfo) = node.fileinfo(users) {
                if path.is_dir() && !path.is_symlink() {
                    if let Some(mut files) =
                        files_collection(&fileinfo, users, show_hidden, filter_kind, true)
                    {
                        sort_kind.sort(&mut files);
                        let children = files
                            .iter()
                            .map(|fileinfo| {
                                stack.push(fileinfo.path.to_owned());
                                fileinfo
                            })
                            .map(|fileinfo| fileinfo.path.to_owned())
                            .collect();
                        node.set_children(Some(children));
                    };
                }
            }
            nodes.insert(node.path.to_owned(), node);
        }

        if let Some(node) = nodes.get_mut(&root_path) {
            node.select()
        }

        Self {
            selected: root_path.clone(),
            root_path,
            nodes,
            required_height: Self::REQUIRED_HEIGHT,
        }
    }

    pub fn empty() -> Self {
        Self {
            root_path: PathBuf::default(),
            selected: PathBuf::default(),
            nodes: HashMap::new(),
            required_height: 0,
        }
    }

    pub fn selected(&self) -> &Path {
        self.selected.as_path()
    }

    pub fn root_path(&self) -> &Path {
        self.root_path.as_path()
    }

    pub fn selected_node(&self) -> Option<&Node> {
        self.nodes.get(self.selected())
    }

    pub fn sort(&mut self, _sort_kind: SortKind) -> Result<()> {
        todo!()
    }

    pub fn select<P>(&mut self, path: P)
    where
        P: AsRef<Path> + Into<PathBuf>,
    {
        self.unselect_current();
        self.selected = path.into();
        self.select_current();
    }

    fn unselect_current(&mut self) {
        if let Some(node) = self.nodes.get_mut(&self.selected) {
            node.unselect()
        }
    }

    fn select_current(&mut self) {
        if let Some(node) = self.nodes.get_mut(&self.selected) {
            node.select()
        }
    }

    /// Select next sibling or the next sibling of the parent
    pub fn select_next(&mut self) -> Result<()> {
        let current_node = self
            .nodes
            .get_mut(&self.selected)
            .context("no selected node")?;

        if let Some(children_paths) = &current_node.children {
            if let Some(child_path) = children_paths.get(0) {
                let child_path = child_path.to_owned();
                current_node.unselect();
                self.selected = child_path.clone();
                self.nodes
                    .get_mut(&child_path)
                    .context("no child path in nodes")?
                    .select();
            }
        } else {
            let mut current_path = self.selected.to_owned();

            while let Some(parent_path) = current_path.parent() {
                let Some(parent_node) = self.nodes.get(parent_path) else {
                    current_path = parent_path.to_owned();
                    continue;
                };
                let Some(siblings_paths) = &parent_node.children else {
                    current_path = parent_path.to_owned();
                    continue;
                };
                let Some(index_current) =
                    siblings_paths.iter().position(|path| path == &current_path)
                else {
                    current_path = parent_path.to_owned();
                    continue;
                };
                let Some(next_sibling_path) = siblings_paths.get(index_current + 1) else {
                    current_path = parent_path.to_owned();
                    continue;
                };
                self.selected = next_sibling_path.to_owned();
                let Some(node) = self.nodes.get_mut(&self.selected) else {
                    current_path = parent_path.to_owned();
                    continue;
                };
                node.select();
                break;
            }
        }
        Ok(())
    }

    /// Select previous sibling or the parent
    pub fn select_prev(&mut self) {
        let current_path = self.selected().to_owned();
        let Some(parent_path) = current_path.parent() else {
            return;
        };
        let Some(parent_node) = self.nodes.get(parent_path) else {
            return;
        };
        let Some(siblings_paths) = &parent_node.children else {
            return;
        };
        let Some(index_current) = siblings_paths.iter().position(|path| path == &current_path)
        else {
            return;
        };
        if index_current > 0 {
            // Previous sibling
            self.selected = siblings_paths[index_current - 1].to_owned();
            let Some(node) = self.nodes.get_mut(&self.selected) else {
                return;
            };
            node.select();
        } else {
            // parent
            let Some(node) = self.nodes.get_mut(parent_path) else {
                return;
            };
            self.selected = parent_path.to_owned();
            node.select();
        }
    }

    /// Fold selected node
    pub fn toggle_fold(&mut self) {
        if let Some(node) = self.nodes.get_mut(&self.selected) {
            node.toggle_fold();
        }
    }

    pub fn toggle_fold_all(&mut self) {
        for (_, node) in self.nodes.iter_mut() {
            node.toggle_fold()
        }
    }

    pub fn search_first_match(&mut self, pattern: &str) {
        if let Some(filename) = self.selected.file_name() {
            let filename = filename.to_string_lossy();
            if filename.contains(pattern) {
                return;
            }
        }
        todo!()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn into_navigable_content(&self, users: &Users) -> (usize, Vec<ColoredTriplet>) {
        let required_height = self.required_height;
        let mut stack = vec![("".to_owned(), self.root_path())];
        let mut content = vec![];
        let mut selected_index = 0;

        while let Some((prefix, current)) = stack.pop() {
            let Some(current_node) = &self.nodes.get(current) else {
                continue;
            };

            if current_node.selected {
                selected_index = content.len();
            }

            let Ok(fileinfo) = FileInfo::new(current, users) else {
                continue;
            };
            let filename = filename_from_path(current).unwrap_or_default().to_owned();

            let mut color_effect = ColorEffect::new(&fileinfo);
            if current_node.selected {
                color_effect.effect |= tuikit::attr::Effect::REVERSE;
            }
            let filename_text = if current.is_dir() && !current.is_symlink() {
                if current_node.folded {
                    format!("▸ {}", filename)
                } else {
                    format!("▾ {}", filename)
                }
            } else {
                filename
            };
            content.push((
                fileinfo.format_no_filename().unwrap_or_default(),
                prefix.to_owned(),
                ColoredString::new(filename_text, color_effect, current.to_owned()),
            ));

            if current.is_dir() && !current.is_symlink() && !current_node.folded {
                let first_prefix = first_prefix(prefix.clone());
                let other_prefix = other_prefix(prefix);

                if let Some(children) = &current_node.children {
                    let mut leaves = children.iter();
                    let Some(first_leaf) = leaves.next() else {
                        continue;
                    };
                    stack.push((first_prefix.clone(), first_leaf));

                    for leaf in leaves {
                        stack.push((other_prefix.clone(), leaf));
                    }
                }
            }

            if content.len() > required_height {
                break;
            }
        }
        (selected_index, content)
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

pub fn calculate_tree_window(
    selected_index: usize,
    terminal_height: usize,
    length: usize,
) -> (usize, usize, usize) {
    let top: usize;
    let bottom: usize;
    let window_height = terminal_height - ContentWindow::WINDOW_MARGIN_TOP;
    if selected_index < terminal_height - 1 {
        top = 0;
        bottom = window_height;
    } else {
        let padding = std::cmp::max(10, terminal_height / 2);
        top = selected_index - padding;
        bottom = top + window_height;
    }

    (top, bottom, length)
}
