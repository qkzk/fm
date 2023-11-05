use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;

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
    last_path: PathBuf,
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

        let mut last_path = root_path.to_owned();
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
            last_path = node.path.to_owned();
            nodes.insert(node.path.to_owned(), node);
        }

        if let Some(node) = nodes.get_mut(&root_path) {
            node.select()
        }

        Self {
            selected: root_path.clone(),
            root_path,
            last_path,
            nodes,
            required_height: Self::REQUIRED_HEIGHT,
        }
    }

    pub fn empty() -> Self {
        Self {
            root_path: PathBuf::default(),
            selected: PathBuf::default(),
            last_path: PathBuf::default(),
            nodes: HashMap::new(),
            required_height: 0,
        }
    }

    pub fn selected_path(&self) -> &Path {
        self.selected.as_path()
    }

    pub fn selected_node(&self) -> Option<&Node> {
        self.nodes.get(&self.selected)
    }

    /// Select next sibling or the next sibling of the parent
    pub fn select_next(&mut self) -> Result<()> {
        log::info!("select_next START {sel}", sel = self.selected.display());

        if let Some(next_path) = self.find_next_path() {
            let Some(next_node) = self.nodes.get_mut(&next_path) else {
                return Ok(());
            };
            log::info!("selecting {next_node:?}");
            next_node.select();
            let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
                unreachable!("current_node should be in nodes");
            };
            selected_node.unselect();
            self.selected = next_path;
        }
        Ok(())
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
                log::info!("returning {next_sibling_path:?}");
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
    pub fn select_prev(&mut self) {
        log::info!("select_prev START {sel}", sel = self.selected.display());

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

    pub fn select_root(&mut self) {
        let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
            unreachable!("selected path should be in node")
        };
        selected_node.unselect();
        let Some(root_node) = self.nodes.get_mut(&self.root_path) else {
            unreachable!("root path should be in nodes")
        };
        root_node.select();
        self.selected = self.root_path.to_owned();
    }

    pub fn select_last(&mut self) {
        let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
            unreachable!("selected path should be in node")
        };
        selected_node.unselect();
        let Some(last_node) = self.nodes.get_mut(&self.last_path) else {
            unreachable!("root path should be in nodes")
        };
        last_node.select();
        self.selected = self.last_path.to_owned();
    }

    pub fn select_parent(&mut self) {
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
        }
    }

    pub fn select_from_path(&mut self, clicked_path: &Path) {
        let Some(new_node) = self.nodes.get_mut(clicked_path) else {
            return;
        };
        new_node.select();
        let Some(selected_node) = self.nodes.get_mut(&self.selected) else {
            unreachable!("current_node should be in nodes");
        };
        selected_node.unselect();
        self.selected = clicked_path.to_owned();
    }

    /// Fold selected node
    pub fn toggle_fold(&mut self) {
        if let Some(node) = self.nodes.get_mut(&self.selected) {
            node.toggle_fold();
        }
    }

    pub fn fold_all(&mut self) {
        for (_, node) in self.nodes.iter_mut() {
            node.fold()
        }
    }

    pub fn unfold_all(&mut self) {
        for (_, node) in self.nodes.iter_mut() {
            node.unfold()
        }
    }

    pub fn directory_of_selected(&self) -> Option<&Path> {
        if self.selected.is_dir() && !self.selected.is_symlink() {
            Some(self.selected.as_path())
        } else {
            self.selected.parent()
        }
    }

    // FIX: can only find the first match and nothing else
    pub fn search_first_match(&mut self, pattern: &str) {
        let initial_selected = self.selected.to_owned();
        let Some((found_path, found_node)) = self.nodes.iter_mut().find(|(path, _)| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .contains(pattern)
        }) else {
            return;
        };
        self.selected = found_path.to_owned();
        found_node.select();
        let Some(current_node) = self.nodes.get_mut(&initial_selected) else {
            unreachable!("selected path should be in nodes");
        };
        current_node.unselect();
    }

    pub fn into_navigable_content(&self, users: &Users) -> (usize, Vec<ColoredTriplet>) {
        let required_height = self.required_height;
        let mut stack = vec![("".to_owned(), self.root_path.as_path())];
        let mut content = vec![];
        let mut selected_index = 0;

        while let Some((prefix, current_path)) = stack.pop() {
            let Some(current_node) = &self.nodes.get(current_path) else {
                continue;
            };

            if current_node.selected {
                selected_index = content.len();
            }

            let Ok(fileinfo) = FileInfo::new(current_path, users) else {
                continue;
            };
            let filename = filename_from_path(current_path)
                .unwrap_or_default()
                .to_owned();

            let mut color_effect = ColorEffect::new(&fileinfo);
            if current_node.selected {
                color_effect.effect |= tuikit::attr::Effect::REVERSE;
            }
            let filename_text = if current_path.is_dir() && !current_path.is_symlink() {
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
                ColoredString::new(filename_text, color_effect, current_path.to_owned()),
            ));

            if current_path.is_dir() && !current_path.is_symlink() && !current_node.folded {
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

    pub fn paths(&self) -> Vec<&std::ffi::OsStr> {
        self.nodes
            .keys()
            .filter_map(|path| path.file_name())
            .collect()
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
