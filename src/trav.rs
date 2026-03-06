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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_basic_traversal() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Create known structure
        fs::write(root.join("file_a.txt"), "hello").unwrap(); // 5 bytes
        fs::write(root.join("file_b.txt"), "world!!").unwrap(); // 7 bytes
        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("file_c.txt"), "abc").unwrap(); // 3 bytes

        let tree = traverse_dir(root.to_path_buf()).unwrap();
        let root_node = tree.get_node(tree.root());

        // Root should have 3 children: file_a, file_b, subdir
        assert_eq!(root_node.children.iter().count(), 3);
        // Root size should be 5 + 7 + 3 = 15 bytes
        assert_eq!(root_node.size.load(std::sync::atomic::Ordering::Relaxed), 15);
    }

    #[test]
    fn test_delete_updates_tree() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::write(root.join("file_a.txt"), "hello").unwrap(); // 5 bytes
        fs::write(root.join("file_b.txt"), "world!!").unwrap(); // 7 bytes

        let tree = traverse_dir(root.to_path_buf()).unwrap();
        let root_size_before = tree.get_node(tree.root()).size.load(Ordering::Relaxed);
        assert_eq!(root_size_before, 12);

        // Find file_a's node index
        let file_a_idx = tree.get_node(tree.root())
            .children
            .iter()
            .map(|(_, &idx)| idx)
            .find(|&idx| tree.get_node(idx).path.file_name().unwrap() == "file_a.txt")
            .unwrap();

        let file_a_size = tree.get_node(file_a_idx).size.load(Ordering::Relaxed);

        // Delete from filesystem
        fs::remove_file(tree.get_node(file_a_idx).path.clone()).unwrap();
        // Update tree
        tree.delete_node(file_a_idx, true);

        // File should be marked deleted
        assert!(tree.get_node(file_a_idx).deleted.load(Ordering::Relaxed));
        // File should no longer exist on disk
        assert!(!root.join("file_a.txt").exists());
        // Root size should have decreased
        let root_size_after = tree.get_node(tree.root()).size.load(Ordering::Relaxed);
        assert_eq!(root_size_after, root_size_before - file_a_size);
    }

    #[test]
    fn test_delete_item_permanent() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        fs::write(root.join("file_a.txt"), "hello").unwrap();
        assert!(root.join("file_a.txt").exists());

        let path = root.join("file_a.txt").to_path_buf();
        delete_item(&path, DeleteMode::Permanent).unwrap();

        assert!(!root.join("file_a.txt").exists());
    }

    #[test]
    fn test_delete_item_directory() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        let subdir = root.join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "hello").unwrap();
        assert!(subdir.exists());

        delete_item(&subdir.to_path_buf(), DeleteMode::Permanent).unwrap();

        assert!(!subdir.exists());
    }
    
    // Just checks if trashing removes the file from the cwd, doesn't check 
    // actual trash functionality, that's on the trash module itself.
    #[test]
    fn test_delete_item_trash() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("trash_me.txt");
        fs::write(&path, "bye").unwrap();
        assert!(path.exists());
        let result = delete_item(&path, DeleteMode::Trash);
        assert!(result.is_ok());
        assert!(!path.exists());
    }
}
