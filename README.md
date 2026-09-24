# diskard
### A fast TUI disk usage analyzer with trash/delete functionality and breakdowns by file extension.

An [ncdu](https://dev.yorhel.nl/ncdu) inspired TUI disk usage analyzer, multithreaded for maximum speed.
Features support for native filesystem trash/recycle bin, so you don't have to permanently delete things immediately if you don't want to.

### Changelog 
**v0.1.2:** 
  - added support for manually refreshing the directory when changes are made externallly.
  - added file counts.
  - added a second view that shows a breakdown of the current directory by file extension (very basic for now, only does file extensions rather than grouping file types)

<img width="1264" height="837" alt="image" src="https://github.com/user-attachments/assets/0458ed83-31e1-4768-944e-6f1ae9da7763" />  
<img width="1267" height="832" alt="image" src="https://github.com/user-attachments/assets/ddabe435-21a1-456c-99c3-37c18cd480c5" />


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
