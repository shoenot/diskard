mod tree;
mod trav;
mod tui;

use std::env;
use std::process;
use std::path::PathBuf;
use trav::traverse_dir;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: diskrd <path>");
        process::exit(1);
    }

    let path = PathBuf::from(&args[1]);

    let tree = traverse_dir(path).unwrap();
    
    tui::run_tui(&tree).unwrap();
}
