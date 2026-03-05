use std::{error::Error, path::PathBuf, cmp::Reverse};

pub(crate) struct Node {
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) size: u64, 
    pub(crate) path: PathBuf,
    pub(crate) children: Vec<usize>,
    pub(crate) parent: Option<usize>,
    pub(crate) deleted: bool,
    pub(crate) unable_to_read: bool,
}

pub struct DirTree {
    nodes: Vec<Node>,
    root: usize,
}

impl DirTree {
    pub fn new(name: String, path: PathBuf) -> DirTree {
        let mut tree = DirTree {
            nodes: Vec::new(),
            root: 0
        };
        tree.nodes.push(Node {
            name,
            is_dir: true,
            size: 0,
            path,
            children: Vec::new(),
            parent: None,
            deleted: false,
            unable_to_read: false,
        });
        tree
    }

    pub fn add_node(&mut self, name: String, 
        is_dir: bool, size: u64, path: PathBuf,
        parent_idx: usize) -> usize {
        let new_node_idx = self.nodes.len();
        let new_node = Node {
            name,
            is_dir,
            size,
            path,
            children: Vec::new(),
            parent: Some(parent_idx),
            deleted: false,
            unable_to_read: false,
        };
        self.nodes.push(new_node);
        self.nodes[parent_idx].children.push(new_node_idx);
        new_node_idx
    }

    pub fn set_size(&mut self, idx: usize, size: u64) {
        self.nodes[idx].size = size;
    }

    pub fn get_node(&self, idx: usize) -> &Node {
        &self.nodes[idx]
    }

    pub fn sort_children(&mut self, idx: usize) {
        let mut children = self.nodes[idx].children.clone();
        children.sort_by_key(|child| Reverse(self.nodes[*child].size));
        self.nodes[idx].children = children;
    }

    pub fn set_unable_to_read(&mut self, idx: usize) {
        self.nodes[idx].unable_to_read = true;
    }

    pub fn delete_node(&mut self, idx: usize, propagate_size: bool) {
        let (is_dir, children, parent_idx, node_size) = {
            let node = &mut self.nodes[idx];
            (node.is_dir, node.children.clone(), node.parent, node.size)
        };
        if is_dir {
            for child_idx in children {
                self.delete_node(child_idx, false);
            }
        }
        if propagate_size {
            let mut current = parent_idx;
            while let Some(pidx) = current {
                self.nodes[pidx].size -= node_size;
                current = self.nodes[pidx].parent;
            }
        }
        self.nodes[idx].deleted = true;
    }
}

