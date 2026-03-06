mod tree;
mod trav;
mod tui;

use std::env;
use std::path::PathBuf;
use trav::traverse_dir;

fn main() {
    let mut args = env::args().skip(1);
    let path = match args.next().as_deref() {
        Some("-h") | Some("--help") => {
            println!("Usage: diskard [path]");
            println!("  path  Directory to scan (default: current directory)");
            std::process::exit(0);
        }
        Some(p) => PathBuf::from(p),
        None => env::current_dir().expect("Could not get current directory"),
    };

    let tree = traverse_dir(path).unwrap();
    
    tui::run_tui(&tree).unwrap();
    std::process::exit(0);
}
