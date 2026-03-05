mod tree;
mod trav;
mod tui;

use std::env;
use std::process;
use std::path::PathBuf;
use tree::DirTree;
use trav::traverse_dir;
use humansize::{format_size, BINARY};

// fn print_tree(tree: &DirTree, node_idx: usize, depth: usize) {
//     let node = tree.get_node(node_idx);
//     let name = &node.name;
//     let size = format_size(node.size.load(std::sync::atomic::Ordering::Relaxed), BINARY);
//     println!("{:indent$}{name} ({size})", "", indent = depth * 2);
//     if node.is_dir { 
//         for (_, child_idx) in &node.children {
//             print_tree(tree, *child_idx, depth + 1);
//         }
//     }
// }

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: diskrd <path>");
        process::exit(1);
    }

    let path = PathBuf::from(&args[1]);

    let tree = traverse_dir(path).unwrap();
    
    tui::run_tui(&tree).unwrap();
    //print_tree(&tree, 0, 0);
}
