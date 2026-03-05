use std::{
    path::PathBuf, 
    sync::atomic::{
        AtomicBool, AtomicU64, Ordering
    }
};
use boxcar::Vec as CVec;

pub(crate) struct Node {
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) size: AtomicU64,
    pub(crate) children: CVec<usize>,
    pub(crate) parent: Option<usize>,
    pub(crate) deleted: AtomicBool,
    pub(crate) unable_to_read: AtomicBool,
}

pub struct DirTree {
    nodes: CVec<Node>,
}

impl DirTree {
    pub fn new(path: PathBuf) -> DirTree {
        let tree = DirTree {
            nodes: CVec::new(),
        };
        tree.nodes.push(Node {
            path,
            is_dir: true,
            size: 0.into(),
            children: CVec::new(),
            parent: None,
            deleted: false.into(),
            unable_to_read: false.into(),
        });
        tree
    }

    pub fn add_node(&self, path: PathBuf, is_dir: bool, size: u64, parent_idx: usize) -> usize {
        let new_node = Node {
            path,
            is_dir,
            size: size.into(),
            children: CVec::new(),
            parent: Some(parent_idx),
            deleted: false.into(),
            unable_to_read: false.into(),
        };
        let new_node_idx = self.nodes.push(new_node);
        self.nodes[parent_idx].children.push(new_node_idx);
        new_node_idx
    }

    pub fn set_size(&self, idx: usize, size: u64) {
        self.nodes[idx].size.store(size, Ordering::Relaxed);
    }

    pub fn get_node(&self, idx: usize) -> &Node {
        &self.nodes[idx]
    }

    pub fn set_unable_to_read(&self, idx: usize) {
        self.nodes[idx].unable_to_read.store(true, Ordering::Relaxed);
    }

    pub fn delete_node(&self, idx: usize, propagate_size: bool) {
        let (is_dir, children, parent_idx, node_size) = {
            let node = &self.nodes[idx];
            (node.is_dir, node.children.clone(), node.parent, node.size.load(Ordering::Relaxed))
        };
        if is_dir {
            for child_idx in children {
                self.delete_node(child_idx, false);
            }
        }
        if propagate_size {
            let mut current = parent_idx;
            while let Some(pidx) = current {
                self.nodes[pidx].size.fetch_sub(node_size, Ordering::Relaxed);
                current = self.nodes[pidx].parent;
            }
        }
        self.nodes[idx].deleted.store(true, Ordering::Relaxed);
    }
}

