use crate::tree::DirTree;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

pub(crate) struct FileTypeTotal {
    pub(crate) extension: String,
    pub(crate) size: u64,
    pub(crate) count: u64,
}

pub(crate) fn breakdown(tree: &DirTree, directory_idx: usize) -> Vec<FileTypeTotal> {
    let mut totals: HashMap<String, (u64, u64)> = HashMap::new();
    let mut pending = vec![directory_idx];

    while let Some(idx) = pending.pop() {
        let node = tree.get_node(idx);

        if node.deleted.load(Ordering::Relaxed) {
            continue;
        }

        if node.is_dir {
            pending.extend(node.children.iter().map(|(_, &child_idx)| child_idx));
            continue;
        }

        let extension = node
            .path
            .extension()
            .filter(|ext| !ext.is_empty())
            .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
            .unwrap_or_else(|| "(no extension)".to_string());

        let total = totals.entry(extension).or_default();
        total.0 += node.size.load(Ordering::Relaxed);
        total.1 += 1;
    }

    let mut rows: Vec<FileTypeTotal> = totals
        .into_iter()
        .map(|(extension, (size, count))| FileTypeTotal {
            extension,
            size,
            count,
        })
        .collect();

    rows.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.extension.cmp(&b.extension))
    });

    rows
}
