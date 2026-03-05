use crate::tree::*;
use std::fs::{
    DirEntry, read_dir, symlink_metadata
};

use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::Ordering,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiskardError {
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Internal Error")]
    InternalError,
}

pub fn traverse_dir(path: PathBuf) -> Result<DirTree, DiskardError> {
    let tree = DirTree::new("root".to_string(), path.clone());
    let ref_tree = Arc::new(tree);
    traverse_recursive(ref_tree.clone(), path, 0);
    match Arc::try_unwrap(ref_tree) {
        Ok(tree) => {
            Ok(tree)
        },
        Err(_) => {
            Err(DiskardError::InternalError)
        }
    }
}

fn traverse_recursive(tree: Arc<DirTree>, path: PathBuf, parent_idx: usize) {
    let reader = match read_dir(path) {
        Ok(r) => r,
        Err(_) => {
            tree.set_unable_to_read(parent_idx);
            return;
        },
    };
    rayon::scope(|s| {
        for item in reader {
            let item: DirEntry = match item {
                Ok(m) => m,
                Err(_) => {
                    tree.set_unable_to_read(parent_idx);
                    continue;
                },
            };
            let item_path = item.path();
            let item_name = item.file_name().to_string_lossy().to_string();
            let metadata = match symlink_metadata(&item_path) {
                Ok(m) => m,
                Err(_) => {
                    tree.set_unable_to_read(parent_idx);
                    continue;
                },
            };
            if metadata.is_file() {
                let item_size = metadata.len();
                tree.add_node(item_name, false, item_size, item_path, parent_idx);
            } else if metadata.is_dir() {
                let item_idx = tree.add_node(item_name, true, 0, item_path.clone(), parent_idx);
                let tc = tree.clone();
                s.spawn(move |_| {
                    traverse_recursive(tc, item_path, item_idx);
                });
            } else {
                continue
            }
        }
    });
    let size: u64 = tree.get_node(parent_idx).children.iter()
        .map(|(_, node_idx)| tree.get_node(*node_idx).size.load(Ordering::Relaxed))
        .sum();
    tree.set_size(parent_idx, size);
}
