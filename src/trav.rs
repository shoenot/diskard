use crate::tree::*;
use std::fs::{
    DirEntry, read_dir, symlink_metadata
};

use std::io::ErrorKind;
use std::path::PathBuf;
use std::error::Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiskardError {
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}

pub fn traverse_dir(path: PathBuf) -> Result<DirTree, DiskardError> {
    let mut tree = DirTree::new("root".to_string(), path.clone());
    let root_size = traverse_recursive(&mut tree, path, 0)?;
    tree.set_size(0, root_size);
    Ok(tree)
}

fn traverse_recursive(tree: &mut DirTree, path: PathBuf, parent_idx: usize) -> Result<u64, DiskardError> {
    let reader = match read_dir(path) {
        Ok(r) => r,
        Err(_) => {
            tree.set_unable_to_read(parent_idx);
            return Ok(0);
        },
    };
    let mut accumulator: u64 = 0;
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
            accumulator += item_size;
        } else if metadata.is_dir() {
            let item_idx = tree.add_node(item_name, true, 0, item_path.clone(), parent_idx);
            let dir_size = traverse_recursive(tree, item_path, item_idx)?;
            tree.set_size(item_idx, dir_size);
            accumulator += dir_size;
        } else {
            continue
        }
    }
    tree.sort_children(parent_idx);
    Ok(accumulator)
}
