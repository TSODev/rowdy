use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::config::ConnectionProfile;

const DB_TYPES: &[&str] = &["postgres", "sqlite", "mysql", "redis"];

pub enum InputMode {
    Normal,
    Editing,
}

pub struct ConnectionScreen {
    pub profiles: Vec<ConnectionProfile>,
    pub list_state: ListState,
    pub input_mode: InputMode,
    pub url_input: String,
    pub db_type_idx: usize,
    pub status: Option<String>,
}

pub enum ConnectionAction {
    None,
    Connect { url: String, db_type: String },
    Quit,
}

impl ConnectionScreen {
    pub fn new(profiles: Vec<ConnectionProfile>) -> Self {
        let mut list_state = ListState::default();
        if !profiles.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            profiles,
            list_state,
            input_mode: InputMode::Normal,
            url_input: String::new(),
            db_type_idx: 0,
            status: None,
        }
    }

    pub fn reset_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.url_input.clear();
        self.status = None;
    }

    pub fn current_db_type(&self) -> &str {
        DB_TYPES[self.db_type_idx]
    }

    pub fn selected_profile(&self) -> Option<&ConnectionProfile> {
        self.list_state.selected().and_then(|i| self.profiles.get(i))
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ConnectionAction {
        match self.input_mode {
            InputMode::Normal => self.handle_normal(key),
            InputMode::Editing => self.handle_editing(key),
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> ConnectionAction {
        match key.code {
            KeyCode::Char('q') => ConnectionAction::Quit,
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                ConnectionAction::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_prev();
                ConnectionAction::None
            }
            KeyCode::Char('n') => {
                self.input_mode = InputMode::Editing;
                self.status = None;
                ConnectionAction::None
            }
            KeyCode::Enter => {
                if let Some(p) = self.selected_profile() {
                    ConnectionAction::Connect {
                        url: p.url.clone(),
                        db_type: p.db_type.clone(),
                    }
                } else {
                    ConnectionAction::None
                }
            }
            _ => ConnectionAction::None,
        }
    }

    fn handle_editing(&mut self, key: KeyEvent) -> ConnectionAction {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                ConnectionAction::None
            }
            KeyCode::Tab => {
                self.db_type_idx = (self.db_type_idx + 1) % DB_TYPES.len();
                ConnectionAction::None
            }
            KeyCode::Enter => {
                if self.url_input.is_empty() {
                    self.status = Some("URL cannot be empty".into());
                    return ConnectionAction::None;
                }
                ConnectionAction::Connect {
                    url: self.url_input.clone(),
                    db_type: self.current_db_type().to_string(),
                }
            }
            KeyCode::Backspace => {
                self.url_input.pop();
                ConnectionAction::None
            }
            KeyCode::Char(c) => {
                self.url_input.push(c);
                ConnectionAction::None
            }
            _ => ConnectionAction::None,
        }
    }

    fn select_next(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        let next = self
            .list_state
            .selected()
            .map_or(0, |i| (i + 1).min(self.profiles.len() - 1));
        self.list_state.select(Some(next));
    }

    fn select_prev(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        let prev = self
            .list_state
            .selected()
            .map_or(0, |i| i.saturating_sub(1));
        self.list_state.select(Some(prev));
    }

    pub fn draw(f: &mut Frame<'_>, screen: &mut ConnectionScreen) {
        let area = f.size();

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area);

        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(outer[0]);

        draw_profiles(f, screen, panels[0]);
        draw_new_connection(f, screen, panels[1]);
        draw_help(f, screen, outer[1]);
    }
}

fn draw_profiles(f: &mut Frame<'_>, screen: &mut ConnectionScreen, area: Rect) {
    let items: Vec<ListItem> = screen
        .profiles
        .iter()
        .map(|p| ListItem::new(format!("[{}]  {}", p.db_type, p.name)))
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Saved Profiles ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut screen.list_state);
}

fn draw_new_connection(f: &mut Frame<'_>, screen: &ConnectionScreen, area: Rect) {
    let is_editing = matches!(screen.input_mode, InputMode::Editing);

    let border_style = if is_editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    f.render_widget(
        Block::default()
            .title(" New Connection ")
            .borders(Borders::ALL)
            .border_style(border_style),
        area,
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    // DB type selector
    let type_text = format!(" < {} >  (Tab to cycle)", screen.current_db_type());
    f.render_widget(
        Paragraph::new(type_text)
            .block(Block::default().title(" Type ").borders(Borders::ALL))
            .style(Style::default().fg(Color::Cyan)),
        inner[0],
    );

    // URL input
    let url_display = if screen.url_input.is_empty() && !is_editing {
        "Press 'n' to enter a URL…".to_string()
    } else {
        screen.url_input.clone()
    };
    let url_style = if is_editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(url_display)
            .block(Block::default().title(" URL ").borders(Borders::ALL))
            .style(url_style),
        inner[1],
    );

    // Place cursor inside the URL input box when editing
    if is_editing {
        f.set_cursor(
            inner[1].x + 1 + screen.url_input.len() as u16,
            inner[1].y + 1,
        );
    }

    // Status / hint
    let hint_text = if let Some(ref msg) = screen.status {
        msg.as_str()
    } else if is_editing {
        "Enter: connect   Esc: cancel   Tab: change type"
    } else {
        "'n' to enter a new connection"
    };
    let hint_style = if screen.status.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(hint_text).style(hint_style),
        inner[2],
    );
}

fn draw_help(f: &mut Frame<'_>, screen: &ConnectionScreen, area: Rect) {
    let text = match screen.input_mode {
        InputMode::Normal =>
            " j/k: move   Enter: connect   n: new connection   q: quit ",
        InputMode::Editing =>
            " Esc: cancel   Tab: change type   Enter: connect   Backspace: delete ",
    };
    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
