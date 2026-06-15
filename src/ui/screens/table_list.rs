use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::db::types::{TableKind, TableObject};

pub enum TableListAction {
    None,
    OpenTable { name: String, is_view: bool },
    OpenEditor,
    Disconnect,
}

pub struct TableListScreen {
    pub tables: Vec<TableObject>,
    pub list_state: ListState,
    pub filter: String,
    pub filter_mode: bool,
    pub status: Option<String>,
    pub db_info: String,
}

impl TableListScreen {
    pub fn new() -> Self {
        Self {
            tables: vec![],
            list_state: ListState::default(),
            filter: String::new(),
            filter_mode: false,
            status: Some("Loading…".into()),
            db_info: String::new(),
        }
    }

    /// Called when a SQL client returns table/view objects.
    pub fn set_tables(&mut self, tables: Vec<TableObject>) {
        self.tables = tables;
        self.status = None;
        if !self.tables.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Called when a KV client returns key names (no VIEW distinction).
    pub fn set_tables_kv(&mut self, names: Vec<String>) {
        self.tables = names.into_iter()
            .map(|name| TableObject { name, kind: TableKind::Table })
            .collect();
        self.status = None;
        if !self.tables.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.status = Some(msg);
    }

    fn filtered(&self) -> Vec<&TableObject> {
        if self.filter.is_empty() {
            self.tables.iter().collect()
        } else {
            let f = self.filter.to_lowercase();
            self.tables.iter().filter(|t| t.name.to_lowercase().contains(&f)).collect()
        }
    }

    fn selected_object(&self) -> Option<(String, bool)> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered().get(i).map(|obj| {
                (obj.name.clone(), obj.kind == TableKind::View)
            }))
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> TableListAction {
        if self.filter_mode {
            return self.handle_filter(key);
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => TableListAction::Disconnect,
            KeyCode::Char('j') | KeyCode::Down  => { self.select_next(); TableListAction::None }
            KeyCode::Char('k') | KeyCode::Up    => { self.select_prev(); TableListAction::None }
            KeyCode::Char('/')                   => { self.filter_mode = true; TableListAction::None }
            KeyCode::Char('e')                   => TableListAction::OpenEditor,
            KeyCode::Enter => {
                if let Some((name, is_view)) = self.selected_object() {
                    TableListAction::OpenTable { name, is_view }
                } else {
                    TableListAction::None
                }
            }
            _ => TableListAction::None,
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> TableListAction {
        match key.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter.clear();
                self.reset_selection();
            }
            KeyCode::Enter => {
                self.filter_mode = false;
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.reset_selection();
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.reset_selection();
            }
            _ => {}
        }
        TableListAction::None
    }

    fn reset_selection(&mut self) {
        let sel = if self.filtered().is_empty() { None } else { Some(0) };
        self.list_state.select(sel);
    }

    fn select_next(&mut self) {
        let len = self.filtered().len();
        if len == 0 { return; }
        let next = self.list_state.selected().map_or(0, |i| (i + 1).min(len - 1));
        self.list_state.select(Some(next));
    }

    fn select_prev(&mut self) {
        let len = self.filtered().len();
        if len == 0 { return; }
        let prev = self.list_state.selected().map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(prev));
    }

    pub fn draw(f: &mut Frame<'_>, screen: &mut TableListScreen, area: Rect) {

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // table list
                Constraint::Length(3), // filter input or help bar
            ])
            .split(area);

        // Collect owned data to release the immutable borrow before render_stateful_widget.
        let (filtered_items, total): (Vec<(String, bool)>, usize) = {
            let v = screen.filtered();
            let items = v.iter().map(|o| (o.name.clone(), o.kind == TableKind::View)).collect();
            (items, screen.tables.len())
        };

        let title = if screen.filter.is_empty() {
            format!(" Tables ({}) ", filtered_items.len())
        } else {
            format!(" Tables ({} / {}) ", filtered_items.len(), total)
        };

        let items: Vec<ListItem> = if let Some(ref msg) = screen.status {
            vec![ListItem::new(msg.as_str()).style(Style::default().fg(Color::DarkGray))]
        } else if filtered_items.is_empty() {
            vec![ListItem::new("No match").style(Style::default().fg(Color::DarkGray))]
        } else {
            filtered_items.iter().map(|(name, is_view)| {
                if *is_view {
                    ListItem::new(Line::from(vec![
                        Span::styled("[V] ", Style::default().fg(Color::Cyan)),
                        Span::raw(name.clone()),
                    ]))
                } else {
                    ListItem::new(Line::from(vec![
                        Span::styled("[T] ", Style::default().fg(Color::DarkGray)),
                        Span::raw(name.clone()),
                    ]))
                }
            }).collect()
        };

        let list = List::new(items)
            .block(Block::default().title(title).borders(Borders::ALL))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        f.render_stateful_widget(list, chunks[0], &mut screen.list_state);

        // Filter bar or help bar
        if screen.filter_mode {
            let filter_display = format!("/{}", screen.filter);
            f.render_widget(
                Paragraph::new(filter_display.clone())
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::Yellow)),
                chunks[1],
            );
            // cursor after the '/' and the typed text
            f.set_cursor(chunks[1].x + 1 + filter_display.len() as u16, chunks[1].y + 1);
        } else {
            f.render_widget(
                Paragraph::new(" j/k: move   Enter: open   e: SQL editor   /: filter   q: disconnect ")
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::DarkGray)),
                chunks[1],
            );
        }
    }
}
