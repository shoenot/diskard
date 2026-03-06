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
    pub(crate) unable_to_read: AtomicBool, }

pub struct DirTree {
    nodes: CVec<Node>,
    root: usize,
}

impl DirTree {
    pub fn new(path: PathBuf) -> DirTree {
        let tree = DirTree {
            nodes: CVec::new(),
            root: 0,
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

    pub fn root(&self) -> usize {
        self.root
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn make_tree() -> DirTree {
        DirTree::new(std::path::PathBuf::from("/test"))
    }

    #[test]
    fn test_add_node_returns_correct_index() {
        let tree = make_tree();
        let idx = tree.add_node("/test/file_a".into(), false, 100, 0);
        assert_eq!(idx, 1);
        let idx2 = tree.add_node("/test/file_b".into(), false, 200, 0);
        assert_eq!(idx2, 2);
    }

    #[test]
    fn test_add_node_wires_parent() {
        let tree = make_tree();
        let idx = tree.add_node("/test/file_a".into(), false, 100, 0);
        let children: Vec<usize> = tree.get_node(0).children.iter().map(|(_, &i)| i).collect();
        assert!(children.contains(&idx));
    }

    #[test]
    fn test_delete_node_sets_deleted_flag() {
        let tree = make_tree();
        let idx = tree.add_node("/test/file_a".into(), false, 100, 0);
        tree.delete_node(idx, false);
        assert!(tree.get_node(idx).deleted.load(Ordering::Relaxed));
    }

    #[test]
    fn test_delete_node_propagates_size() {
        let tree = make_tree();
        tree.set_size(0, 300);
        let idx = tree.add_node("/test/file_a".into(), false, 100, 0);
        tree.delete_node(idx, true);
        assert_eq!(tree.get_node(0).size.load(Ordering::Relaxed), 200);
    }

    #[test]
    fn test_delete_node_recursive_marks_children() {
        let tree = make_tree();
        let dir_idx = tree.add_node("/test/subdir".into(), true, 0, 0);
        let child_idx = tree.add_node("/test/subdir/file".into(), false, 50, dir_idx);
        tree.delete_node(dir_idx, false);
        assert!(tree.get_node(dir_idx).deleted.load(Ordering::Relaxed));
        assert!(tree.get_node(child_idx).deleted.load(Ordering::Relaxed));
    }

    #[test]
    fn test_delete_node_propagates_size_through_ancestors() {
        let tree = make_tree();
        tree.set_size(0, 150);
        let dir_idx = tree.add_node("/test/subdir".into(), true, 100, 0);
        let child_idx = tree.add_node("/test/subdir/file".into(), false, 100, dir_idx);
        tree.delete_node(child_idx, true);
        // Both dir and root should have size reduced
        assert_eq!(tree.get_node(dir_idx).size.load(Ordering::Relaxed), 0);
        assert_eq!(tree.get_node(0).size.load(Ordering::Relaxed), 50);
    }
}

