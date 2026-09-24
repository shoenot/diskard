use crate::tree::DirTree;
use crate::trav::traverse_dir;
use crate::filetype::{breakdown, FileTypeTotal};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

enum Modal {
    None,
    ConfirmTrash(usize),
    ConfirmDelete(usize),
    Error(String),
}

pub struct App {
    tree: DirTree,
    nav_stack: Vec<(usize, usize)>,
    list_state: ListState,
    modal: Modal,
    show_types: bool,
    type_rows: Vec<FileTypeTotal>,
    type_list_state: ListState,
    refreshing: bool,
}

impl App {
    pub fn new(tree: DirTree) -> Self {
        let root = tree.root();
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            tree,
            nav_stack: vec![(root, 0)],
            list_state,
            modal: Modal::None,
            show_types: false,
            type_rows: Vec::new(),
            type_list_state: ListState::default(),
            refreshing: false,
        }
    }

    fn current_node_idx(&self) -> usize {
        self.nav_stack.last().unwrap().0
    }

    fn selected_idx(&self) -> usize {
        self.nav_stack.last().unwrap().1
    }

    fn children(&self) -> Vec<usize> {
        visible_children(&self.tree, self.current_node_idx())
    }

    fn move_up(&mut self) {
        let selected = self.selected_idx();
        let new_selected = selected.saturating_sub(1);
        let last = self.nav_stack.last_mut().unwrap();
        last.1 = new_selected;
        self.list_state.select(Some(new_selected));
    }

    fn move_down(&mut self) {
        let children_len = self.children().len();
        if children_len == 0 { return; }
        let selected = self.selected_idx();
        let new_selected = (selected + 1).min(children_len - 1);
        let last = self.nav_stack.last_mut().unwrap();
        last.1 = new_selected;
        self.list_state.select(Some(new_selected));
    }

    fn enter(&mut self) {
        let children = self.children();
        if children.is_empty() { return; }
        let selected = self.selected_idx();
        if selected >= children.len() { return; }
        let child_idx = children[selected];
        let node = self.tree.get_node(child_idx);
        if node.is_dir {
            self.nav_stack.push((child_idx, 0));
            self.list_state.select(Some(0));
        }
    }

    fn go_back(&mut self) {
        if self.nav_stack.len() > 1 {
            self.nav_stack.pop();
            let selected = self.selected_idx();
            self.list_state.select(Some(selected));
        }
    }

    fn prompt_trash(&mut self) {
        let children = self.children();
        if children.is_empty() { return; }
        let selected = self.selected_idx();
        if selected >= children.len() { return; }
        self.modal = Modal::ConfirmTrash(children[selected]);
    }

    fn prompt_delete(&mut self) {
        let children = self.children();
        if children.is_empty() { return; }
        let selected = self.selected_idx();
        if selected >= children.len() { return; }
        self.modal = Modal::ConfirmDelete(children[selected]);
    }

    fn confirm_action(&mut self) {
        match &self.modal {
            Modal::ConfirmTrash(idx) => {
                let idx = *idx;
                let path = self.tree.get_node(idx).path.clone();
                match trash::delete(&path) {
                    Ok(_) => {
                        self.tree.delete_node(idx, true);
                        self.adjust_selection();
                        self.modal = Modal::None;
                    }
                    Err(e) => {
                        self.modal = Modal::Error(format!("Trash failed: {}", e));
                    }
                }
            }
            Modal::ConfirmDelete(idx) => {
                let idx = *idx;
                let node = self.tree.get_node(idx);
                let path = node.path.clone();
                let result = if node.is_dir {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                };
                match result {
                    Ok(_) => {
                        self.tree.delete_node(idx, true);
                        self.adjust_selection();
                        self.modal = Modal::None;
                    }
                    Err(e) => {
                        self.modal = Modal::Error(format!("Delete failed: {}", e));
                    }
                }
            }
            _ => {}
        }
    }

    fn cancel_modal(&mut self) {
        self.modal = Modal::None;
    }

    fn adjust_selection(&mut self) {
        let new_len = self.children().len();
        let selected = self.selected_idx();
        if new_len == 0 {
            let last = self.nav_stack.last_mut().unwrap();
            last.1 = 0;
            self.list_state.select(Some(0));
        } else {
            let new_selected = selected.min(new_len - 1);
            let last = self.nav_stack.last_mut().unwrap();
            last.1 = new_selected;
            self.list_state.select(Some(new_selected));
        }
    }

    fn open_types(&mut self) {
        self.type_rows = breakdown(&self.tree, self.current_node_idx());
        self.type_list_state.select((!self.type_rows.is_empty()).then_some(0));
        self.show_types = true;
    }

    fn type_up(&mut self) {
        if let Some(selected) = self.type_list_state.selected() {
            self.type_list_state.select(Some(selected.saturating_sub(1)));
        }
    }

    fn type_down(&mut self) {
        if let Some(selected) = self.type_list_state.selected() {
            let last = self.type_rows.len() - 1;
            self.type_list_state.select(Some((selected + 1).min(last)));
        }
    }

    fn refresh(&mut self) {
        // save directory paths and selected child paths before indices become invalid.
        let saved: Vec<(PathBuf, Option<PathBuf>, usize)> = self.nav_stack.iter()
            .map(|&(dir_idx, row)| {
                let directory_path = self.tree.get_node(dir_idx).path.clone();
                let selected_path = visible_children(&self.tree, dir_idx)
                    .get(row)
                    .map(|&child_idx| self.tree.get_node(child_idx).path.clone());
    
                (directory_path, selected_path, row)
            })
            .collect();
    
        let root_path = self.tree.get_node(self.tree.root()).path.clone();
        let new_tree = match traverse_dir(root_path) {
            Ok(tree) => tree,
            Err(error) => {
                self.modal = Modal::Error(format!("Refresh failed: {error}"));
                return;
            }
        };
    
        let mut new_stack = Vec::with_capacity(saved.len());
        let mut current_idx = new_tree.root();
    
        for (level, (directory_path, selected_path, old_row)) in
            saved.iter().enumerate()
        {
            if new_tree.get_node(current_idx).path.as_path()
                != directory_path.as_path()
            {
                break;
            }
    
            let children = visible_children(&new_tree, current_idx);
            let selected_row = selected_path
                .as_ref()
                .and_then(|wanted| {
                    children.iter().position(|&child_idx| {
                        new_tree.get_node(child_idx).path.as_path() == wanted.as_path()
                    })
                })
                .unwrap_or_else(|| (*old_row).min(children.len().saturating_sub(1)));
    
            new_stack.push((current_idx, selected_row));
    
            let Some((next_directory_path, _, _)) = saved.get(level + 1) else {
                break;
            };
    
            let Some(next_idx) = children.into_iter().find(|&child_idx| {
                let child = new_tree.get_node(child_idx);
                child.is_dir && child.path.as_path() == next_directory_path.as_path()
            }) else {
                break;
            };
    
            current_idx = next_idx;
        }
    
        if new_stack.is_empty() {
            new_stack.push((new_tree.root(), 0));
        }
    
        self.tree = new_tree;
        self.nav_stack = new_stack;
    
        let selection = if self.children().is_empty() {
            None
        } else {
            Some(self.selected_idx())
        };
        self.list_state.select(selection);
    
        self.type_rows.clear();
        self.type_list_state.select(None);
        self.modal = Modal::None;
    }
}

pub fn run_tui(tree: DirTree) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(tree);
    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{:.0} {}", size, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if app.show_types {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('t') | KeyCode::Esc => app.show_types = false,
                    KeyCode::Up | KeyCode::Char('k') => app.type_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.type_down(),
                    _ => {}
                }
                continue;
            }

            match &app.modal {
                Modal::None => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => app.enter(),
                    KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') => app.go_back(),
                    KeyCode::Char('t') => app.open_types(),
                    KeyCode::Char('d') => app.prompt_trash(),
                    KeyCode::Char('D') => app.prompt_delete(),
                    KeyCode::Char('r') => {
                        app.refreshing = true;
                        terminal.draw(|f| ui(f, app))?;
                        app.refresh();
                        app.refreshing = false;
                    },
                    _ => {}
                },
                Modal::ConfirmTrash(_) | Modal::ConfirmDelete(_) => match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => app.confirm_action(),
                    KeyCode::Char('n') | KeyCode::Esc => app.cancel_modal(),
                    _ => {}
                },
                Modal::Error(_) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter => app.cancel_modal(),
                    _ => {}
                },
            }
        }
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
    if app.show_types { ui_types(f, app); return; }
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    let current_node = app.tree.get_node(app.current_node_idx());
    let header_text = format!("{} ({} files)", current_node.path.to_string_lossy(), current_node.file_count.load(Ordering::Relaxed));
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(header, chunks[0]);

    // List
    let children = app.children();
    let total_size = current_node.size.load(Ordering::Relaxed);
    
    let width = chunks[1].width.saturating_sub(4) as usize;
    let size_col = if width >= 20 { 10 } else { 0 };
    let percent_col = if width >= 33 { 7 } else { 0 };
    let count_col = if width >= 52 { 12 } else { 0 };
    let bar_col = if width >= 76 { width / 5 } else { 0 };
    let name_col = width.saturating_sub(size_col + percent_col + count_col + bar_col + usize::from(bar_col > 0));
    
    let items: Vec<ListItem> = children.iter()
        .map(|&idx| {
            let node = app.tree.get_node(idx);
            let size = node.size.load(Ordering::Relaxed);
            let share = if total_size == 0 { 0.0 } else { size as f64 / total_size as f64 };
    
            let full_name = node
                .path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".to_string());
    
            let name = if name_col == 0 {
                String::new()
            } else if full_name.chars().count() > name_col {
                format!(
                    "{}…",
                    full_name
                        .chars()
                        .take(name_col.saturating_sub(1))
                        .collect::<String>()
                )
            } else {
                format!("{:<width$}", full_name, width = name_col)
            };
    
            let color = if node.unable_to_read.load(Ordering::Relaxed) {
                Color::Red
            } else if node.is_dir {
                Color::Blue
            } else {
                Color::White
            };
    
            let mut spans = Vec::new();
            
            if size_col > 0 {
                spans.push(Span::styled(
                    format!("{:>width$} ", format_size(size), width = size_col - 1),
                    Style::default().fg(Color::Yellow),
                ));
            }
            
            spans.push(Span::styled(name, Style::default().fg(color)));
    
            if percent_col > 0 {
                spans.push(Span::raw(format!(
                    "{:>width$} ",
                    format!("{:.1}%", share * 100.0),
                    width = percent_col - 1,
                )));
            }
    
            if count_col > 0 {
                let count_text = if node.is_dir {
                    format!("{} files", node.file_count.load(Ordering::Relaxed))
                } else {
                    String::new()
                };
    
                spans.push(Span::styled(
                    format!("{:>width$} ", count_text, width = count_col - 1),
                    Style::default().fg(Color::DarkGray),
                ));
            }
    
            if bar_col > 0 {
                let filled = (share * bar_col as f64).round() as usize;
                spans.push(Span::styled(
                    format!(" {}", "█".repeat(filled.min(bar_col))),
                    Style::default().fg(color),
                ));
            }
    
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::Black).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Footer
    let footer_text = if app.refreshing { " Refreshing..." } else {
        match &app.modal {
            Modal::None => " ↑/k: up  ↓/j: down  ->/l/Enter: open  <-/h/Backspace: back  t: types  r: refresh  d: trash  D: delete  q: quit",
            Modal::ConfirmTrash(_) | Modal::ConfirmDelete(_) => {
                " y/Enter: confirm  n/Esc: cancel"
            }
            Modal::Error(_) => " Enter/Esc: dismiss",
        }
    };
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[2]);

    // Modal overlays
    match &app.modal {
        Modal::ConfirmTrash(idx) | Modal::ConfirmDelete(idx) => {
            let is_trash = matches!(&app.modal, Modal::ConfirmTrash(_));
            let node = app.tree.get_node(*idx);
            let name = node.path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".to_string());
            let action = if is_trash { "Move to trash" } else { "Permanently delete" };
            let msg = format!(" {} \"{}\"? ", action, name);
            let width = (msg.len() as u16 + 4).max(36).min(area.width - 4);
            let dialog_area = centered_rect(width, 5, area);

            let color = if is_trash { Color::Yellow } else { Color::Red };
            let dialog = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(msg, Style::default().fg(Color::White))),
                Line::from(Span::styled(
                    " Press y to confirm, n to cancel",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color))
                    .title(Span::styled(
                        if is_trash { " Trash " } else { " Delete " },
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    )),
            );

            f.render_widget(Clear, dialog_area);
            f.render_widget(dialog, dialog_area);
        }
        Modal::Error(msg) => {
            let msg = msg.clone();
            let width = (msg.len() as u16 + 4).max(40).min(area.width - 4);
            let dialog_area = centered_rect(width, 5, area);
            let dialog = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!(" {} ", msg),
                    Style::default().fg(Color::White),
                )),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red))
                    .title(Span::styled(
                        " Error ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )),
            );

            f.render_widget(Clear, dialog_area);
            f.render_widget(dialog, dialog_area);
        }
        Modal::None => {}
    }
}

fn ui_types(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    let directory = app.tree.get_node(app.current_node_idx());
    let total_size = directory.size.load(Ordering::Relaxed);
    let file_count = directory.file_count.load(Ordering::Relaxed);

    let header = Paragraph::new(format!(
        "File types: {} ({} files)",
        directory.path.to_string_lossy(),
        file_count,
    ))
    .block(Block::default().borders(Borders::ALL))
    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(header, chunks[0]);

    let width = chunks[1].width.saturating_sub(4) as usize;
    let size_col = if width >= 20 { 10 } else { 0 };
    let percent_col = if width >= 33 { 7 } else { 0 };
    let count_col = if width >= 52 { 12 } else { 0 };
    let bar_col = if width >= 76 { width / 5 } else { 0 };
    let type_col = width.saturating_sub(
        size_col + percent_col + count_col + bar_col + usize::from(bar_col > 0),
    );

    let items: Vec<ListItem> = app
        .type_rows
        .iter()
        .map(|row| {
            let name = if type_col == 0 {
                String::new()
            } else if row.extension.chars().count() > type_col {
                format!(
                    "{}…",
                    row.extension
                        .chars()
                        .take(type_col.saturating_sub(1))
                        .collect::<String>()
                )
            } else {
                format!("{:<width$}", row.extension, width = type_col)
            };

            let share = if total_size == 0 {
                0.0
            } else {
                row.size as f64 / total_size as f64
            };

            let mut spans = Vec::new();
            
            if size_col > 0 {
                spans.push(Span::styled(
                    format!("{:>width$} ", format_size(row.size), width = size_col - 1),
                    Style::default().fg(Color::Yellow),
                ));
            }
            
            spans.push(Span::styled(name, Style::default().fg(Color::Blue)));

            if percent_col > 0 {
                spans.push(Span::raw(format!(
                    "{:>width$} ",
                    format!("{:.1}%", share * 100.0),
                    width = percent_col - 1,
                )));
            }

            if count_col > 0 {
                spans.push(Span::styled(
                    format!(
                        "{:>width$} ",
                        format!("{} files", row.count),
                        width = count_col - 1,
                    ),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            if bar_col > 0 {
                let filled = (share * bar_col as f64).round() as usize;
                spans.push(Span::styled(
                    format!(" {}", "█".repeat(filled.min(bar_col))),
                    Style::default().fg(Color::Blue),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Extensions by size "))
        .highlight_style(Style::default().bg(Color::Black).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[1], &mut app.type_list_state);

    let footer = Paragraph::new(" ↑/k: up  ↓/j: down  t/Esc: back  q: quit ")
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, chunks[2]);
}

fn visible_children(tree: &DirTree, parent_idx: usize) -> Vec<usize> {
    let node = tree.get_node(parent_idx);
    let mut children: Vec<usize> = node
        .children
        .iter()
        .map(|(_, &idx)| idx)
        .filter(|&idx| !tree.get_node(idx).deleted.load(Ordering::Relaxed))
        .collect();

    children.sort_by(|&a, &b| {
        tree.get_node(b)
            .size
            .load(Ordering::Relaxed)
            .cmp(&tree.get_node(a).size.load(Ordering::Relaxed))
    });

    children
}
