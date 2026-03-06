# diskard
### A fast TUI disk usage analyzer with trash/delete functionality.

An [ncdu](https://dev.yorhel.nl/ncdu) inspired TUI disk usage analyzer, multithreaded for maximum speed.
Features support for native filesystem trash/recycle bin, so you don't have to permanently delete things immediately if you don't want to.

<img width="1219" height="638" alt="image" src="https://github.com/user-attachments/assets/ccc9b8e8-336a-4107-8216-c7f9fcf7365c" />

---

### Installation:
- Clone the repo
- cd into the repo 
- Run ```cargo install --path .```

### Arch Linux
There is now an AUR package available for diskard at [this link](https://aur.archlinux.org/packages/diskard). Install by cloning the AUR repo and running:   
```makepkg -si```    
(Or use your favorite AUR helper eg ```yay -S diskard```)

---

### Usage:  
```diskard [path]```  
  
(If no path is provided, the program uses the current working directory)
