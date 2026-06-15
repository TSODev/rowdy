use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

pub enum TableListAction {
    None,
    OpenTable(String),
    OpenEditor,
    Disconnect,
}

pub struct TableListScreen {
    pub tables: Vec<String>,
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

    pub fn set_tables(&mut self, tables: Vec<String>) {
        self.tables = tables;
        self.status = None;
        if !self.tables.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.status = Some(msg);
    }

    fn filtered(&self) -> Vec<&String> {
        if self.filter.is_empty() {
            self.tables.iter().collect()
        } else {
            let f = self.filter.to_lowercase();
            self.tables.iter().filter(|t| t.to_lowercase().contains(&f)).collect()
        }
    }

    fn selected_name(&self) -> Option<String> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered().get(i).map(|s| s.to_string()))
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
                if let Some(name) = self.selected_name() {
                    TableListAction::OpenTable(name)
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

        // Table list — collect owned Strings to release the immutable borrow
        // before we need &mut screen.list_state for render_stateful_widget.
        let (filtered_names, total): (Vec<String>, usize) = {
            let v = screen.filtered();
            let names = v.iter().map(|s| s.to_string()).collect();
            (names, screen.tables.len())
        };

        let title = if screen.filter.is_empty() {
            format!(" Tables ({}) ", filtered_names.len())
        } else {
            format!(" Tables ({} / {}) ", filtered_names.len(), total)
        };

        let items: Vec<ListItem> = if let Some(ref msg) = screen.status {
            vec![ListItem::new(msg.as_str()).style(Style::default().fg(Color::DarkGray))]
        } else if filtered_names.is_empty() {
            vec![ListItem::new("No match").style(Style::default().fg(Color::DarkGray))]
        } else {
            filtered_names.iter().map(|t| ListItem::new(t.as_str())).collect()
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
