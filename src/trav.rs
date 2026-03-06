use crate::tree::*;
use std::fs::{
    DirEntry, read_dir,
};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::Ordering,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiskardError {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Internal Error")]
    InternalError,
    #[error("Delete Failed: {0}")]
    DeleteFailed(String),
}

pub enum DeleteMode {
    Trash,
    Permanent,
}

pub fn traverse_dir(path: PathBuf) -> Result<DirTree, DiskardError> {
    let tree = DirTree::new(path.clone());
    let ref_tree = Arc::new(tree);
    let root = ref_tree.root();
    traverse_recursive(ref_tree.clone(), path, root);
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
            let file_type = match item.file_type() {
                Ok(ft) => ft,
                Err(_) => {
                    tree.set_unable_to_read(parent_idx);
                    continue;
                },
            };
            if file_type.is_symlink() {
                continue
            } else if file_type.is_file() {
                let item_size = match item.metadata() {
                    Ok(m) => m.len(),
                    Err(_) => continue,
                };
                tree.add_node(item_path, false, item_size, parent_idx);
            } else if file_type.is_dir() {
                let item_idx = tree.add_node(item_path.clone(), true, 0, parent_idx);
                let tc = tree.clone();
                s.spawn(move |_| {
                    traverse_recursive(tc, item_path, item_idx);
                });
            } 
        }
    });
    let size: u64 = tree.get_node(parent_idx).children.iter()
        .map(|(_, node_idx)| tree.get_node(*node_idx).size.load(Ordering::Relaxed))
        .sum();
    tree.set_size(parent_idx, size);
}

pub fn delete_item(path: &PathBuf, mode: DeleteMode) -> Result<(), DiskardError> {
    match mode {
        DeleteMode::Trash => trash::delete(path).map_err(|e| DiskardError::DeleteFailed(e.to_string())),
        DeleteMode::Permanent => {
            if path.is_dir() {
                std::fs::remove_dir_all(path).map_err(|e| DiskardError::DeleteFailed(e.to_string()))
            } else {
                std::fs::remove_file(path).map_err(|e| DiskardError::DeleteFailed(e.to_string()))
            }
        }
    }
}
