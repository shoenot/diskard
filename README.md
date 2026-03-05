# Diskard
## A fast TUI disk usage analyzer with trash/delete functionality.

An [ncdu](https://dev.yorhel.nl/ncdu) inspired TUI disk usage analyzer, multithreaded for maximum speed.
Features support for native filesystem trash/recycle bin, so you don't have to permanently delete things immediately if you don't want to.

Installation:
- Clone the repo
- cd into the repo 
- Run ```cargo build --release```
- Copy ./target/release/diskard to somewhere in your $PATH.

Usage: 
diskard [path] (If no path is provided, the program uses the current working directory)
