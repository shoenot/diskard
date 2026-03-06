use crate::{
    trav::{
        DeleteMode, 
        delete_item,
    }, 
    tree::DirTree,
};
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
use std::sync::atomic::Ordering;

enum Modal {
    None,
    ConfirmTrash(usize),
    ConfirmDelete(usize),
    Error(String),
}

pub struct App<'a> {
    tree: &'a DirTree,
    nav_stack: Vec<(usize, usize)>,
    list_state: ListState,
    modal: Modal,
}

impl<'a> App<'a> {
    pub fn new(tree: &'a DirTree) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            tree,
            nav_stack: vec![(tree.root(), 0)],
            list_state,
            modal: Modal::None,
        }
    }

    fn current_node_idx(&self) -> usize {
        self.nav_stack.last().unwrap().0
    }

    fn selected_idx(&self) -> usize {
        self.nav_stack.last().unwrap().1
    }

    fn children(&self) -> Vec<usize> {
        let node = self.tree.get_node(self.current_node_idx());
        let mut children: Vec<usize> = node
            .children
            .iter()
            .map(|(_, &idx)| idx)
            .filter(|&idx| !self.tree.get_node(idx).deleted.load(Ordering::Relaxed))
            .collect();
        children.sort_by(|&a, &b| {
            self.tree.get_node(b).size.load(Ordering::Relaxed)
                .cmp(&self.tree.get_node(a).size.load(Ordering::Relaxed))
        });
        children
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
                match delete_item(&path, DeleteMode::Trash) {
                    Ok(_) => {
                        self.tree.delete_node(idx, true);
                        self.adjust_selection();
                        self.modal = Modal::None;
                    }
                    Err(e) => {
                        self.modal = Modal::Error(e.to_string());
                    }
                }
            }
            Modal::ConfirmDelete(idx) => {
                let idx = *idx;
                let node = self.tree.get_node(idx);
                let path = node.path.clone();
                match delete_item(&path, DeleteMode::Permanent) {
                    Ok(_) => {
                        self.tree.delete_node(idx, true);
                        self.adjust_selection();
                        self.modal = Modal::None;
                    }
                    Err(e) => {
                        self.modal = Modal::Error(e.to_string());
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
}

pub fn run_tui(tree: &DirTree) -> io::Result<()> {
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
            if key.kind == KeyEventKind::Press {
                match &app.modal {
                    Modal::None => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                        KeyCode::Right | KeyCode::Enter | KeyCode::Char('l') => app.enter(),
                        KeyCode::Left |KeyCode::Backspace | KeyCode::Char('h') => app.go_back(),
                        KeyCode::Char('d') => app.prompt_trash(),
                        KeyCode::Char('D') => app.prompt_delete(),
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
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn ui(f: &mut ratatui::Frame, app: &mut App) {
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
    let header_text = current_node.path.to_string_lossy().to_string();
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(header, chunks[0]);

    // List
    let children = app.children();
    let max_size = children.iter()
        .map(|&idx| app.tree.get_node(idx).size.load(Ordering::Relaxed))
        .max()
        .unwrap_or(1)
        .max(1);

    let total = chunks[1].width.saturating_sub(4) as usize;
    let size_col = 10;
    let bar_col = (total * 25) / 100;
    let name_col = total.saturating_sub(size_col + bar_col + 2);

    let items: Vec<ListItem> = children
        .iter()
        .map(|&idx| {
            let node = app.tree.get_node(idx);
            let size = node.size.load(Ordering::Relaxed);
            let size_str = format_size(size);

            let full_name = node.path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".to_string());
            let name = if full_name.chars().count() > name_col {
                format!("{}…", full_name.chars().take(name_col.saturating_sub(1)).collect::<String>())
            } else {
                format!("{:<width$}", full_name, width = name_col)
            };

            let filled = ((size as f64 / max_size as f64) * bar_col as f64) as usize;
            let bar = format!("{:<width$}", "█".repeat(filled), width = bar_col);

            let color = if node.unable_to_read.load(Ordering::Relaxed) {
                Color::Red
            } else if node.is_dir {
                Color::Blue
            } else {
                Color::White
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{:>width$} ", size_str, width = size_col - 1),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(name, Style::default().fg(color)),
                Span::styled(format!(" {}", bar), Style::default().fg(color)),
            ]);

            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::Black).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[1], &mut app.list_state);

    // Footer
    let footer_text = match &app.modal {
        Modal::None => " ↑/k: up  ↓/j: down  ->/l/Enter: open  <-/h/Backspace: back  d: trash  D: delete  q: quit",
        Modal::ConfirmTrash(_) | Modal::ConfirmDelete(_) => " y/Enter: confirm  n/Esc: cancel",
        Modal::Error(_) => " Enter/Esc: dismiss",
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
