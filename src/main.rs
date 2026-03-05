mod tree;
mod trav;
mod tui;

use std::env;
use std::path::PathBuf;
use trav::traverse_dir;
use clap::{
    Parser
};

#[derive(Parser)]
#[command(name = "diskard", about="A fast terminal disk usage analyzer with trash/delete capabilities.")]
struct Args {
    path: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    let path = args.path.unwrap_or_else(|| {
        env::current_dir().expect("Could not get current directory.")
    });

    let tree = traverse_dir(path).unwrap();
    
    tui::run_tui(&tree).unwrap();
}
