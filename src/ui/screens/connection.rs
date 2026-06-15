use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::config::ConnectionProfile;

const DB_TYPES: &[&str] = &["postgres", "sqlite", "libsql", "mysql", "redis"];

pub enum InputMode {
    Normal,
    Editing,
    SavingName,
    ConfirmDelete,
}

pub struct ConnectionScreen {
    pub profiles: Vec<ConnectionProfile>,
    pub list_state: ListState,
    pub input_mode: InputMode,
    pub url_input: String,
    pub name_input: String,
    pub db_type_idx: usize,
    pub status: Option<String>,
    pub pending_delete: Option<usize>,
}

pub enum ConnectionAction {
    None,
    Connect { url: String, db_type: String },
    SaveProfile { name: String, url: String, db_type: String },
    DeleteProfile { idx: usize, persist: bool },
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
            name_input: String::new(),
            db_type_idx: 0,
            status: None,
            pending_delete: None,
        }
    }

    pub fn reset_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.url_input.clear();
        self.name_input.clear();
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
            InputMode::SavingName => self.handle_saving_name(key),
            InputMode::ConfirmDelete => self.handle_confirm_delete(key),
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
                self.url_input.clear();
                self.name_input.clear();
                self.status = None;
                ConnectionAction::None
            }
            KeyCode::Char('D') | KeyCode::Delete => {
                if let Some(i) = self.list_state.selected() {
                    if i < self.profiles.len() {
                        self.pending_delete = Some(i);
                        self.input_mode = InputMode::ConfirmDelete;
                        self.status = None;
                    }
                }
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
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            if key.code == KeyCode::Char('s') {
                if self.url_input.is_empty() {
                    self.status = Some("Enter a URL first".into());
                } else {
                    self.name_input.clear();
                    self.input_mode = InputMode::SavingName;
                    self.status = None;
                }
                return ConnectionAction::None;
            }
        }
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

    fn handle_confirm_delete(&mut self, key: KeyEvent) -> ConnectionAction {
        let idx = match self.pending_delete {
            Some(i) => i,
            None => {
                self.input_mode = InputMode::Normal;
                return ConnectionAction::None;
            }
        };
        match key.code {
            KeyCode::Char('y') => {
                self.pending_delete = None;
                self.input_mode = InputMode::Normal;
                ConnectionAction::DeleteProfile { idx, persist: true }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.pending_delete = None;
                self.input_mode = InputMode::Normal;
                ConnectionAction::DeleteProfile { idx, persist: false }
            }
            _ => ConnectionAction::None,
        }
    }

    fn handle_saving_name(&mut self, key: KeyEvent) -> ConnectionAction {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Editing;
                self.status = None;
                ConnectionAction::None
            }
            KeyCode::Enter => {
                if self.name_input.is_empty() {
                    self.status = Some("Name cannot be empty".into());
                    return ConnectionAction::None;
                }
                let action = ConnectionAction::SaveProfile {
                    name: self.name_input.clone(),
                    url: self.url_input.clone(),
                    db_type: self.current_db_type().to_string(),
                };
                self.name_input.clear();
                self.input_mode = InputMode::Normal;
                action
            }
            KeyCode::Backspace => {
                self.name_input.pop();
                ConnectionAction::None
            }
            KeyCode::Char(c) => {
                self.name_input.push(c);
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
    let pending = screen.pending_delete;
    let items: Vec<ListItem> = screen
        .profiles
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let text = format!("[{}]  {}", p.db_type, p.name);
            if pending == Some(i) {
                ListItem::new(text).style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            } else {
                ListItem::new(text)
            }
        })
        .collect();

    let is_confirm = matches!(screen.input_mode, InputMode::ConfirmDelete);
    let highlight_style = if is_confirm {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    };

    let list = List::new(items)
        .block(Block::default().title(" Saved Profiles ").borders(Borders::ALL))
        .highlight_style(highlight_style)
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut screen.list_state);
}

fn draw_new_connection(f: &mut Frame<'_>, screen: &ConnectionScreen, area: Rect) {
    let is_editing = matches!(screen.input_mode, InputMode::Editing);
    let is_saving = matches!(screen.input_mode, InputMode::SavingName);

    let border_style = if is_editing || is_saving {
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
            Constraint::Length(3), // Type
            Constraint::Length(3), // URL
            Constraint::Length(3), // Name (for save)
            Constraint::Min(0),    // Hint
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
    let url_display = if screen.url_input.is_empty() && !is_editing && !is_saving {
        "Press 'n' to enter a URL…".to_string()
    } else {
        screen.url_input.clone()
    };
    let url_style = if is_editing {
        Style::default().fg(Color::Yellow)
    } else if is_saving {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(url_display)
            .block(Block::default().title(" URL ").borders(Borders::ALL))
            .style(url_style),
        inner[1],
    );

    // Name input (visible when saving)
    let name_display = if screen.name_input.is_empty() && !is_saving {
        "Ctrl+S to save with a name…".to_string()
    } else {
        screen.name_input.clone()
    };
    let name_style = if is_saving {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let name_block_style = if is_saving {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(name_display)
            .block(
                Block::default()
                    .title(" Save as (name) ")
                    .borders(Borders::ALL)
                    .border_style(name_block_style),
            )
            .style(name_style),
        inner[2],
    );

    // Cursor positioning
    if is_editing {
        f.set_cursor(
            inner[1].x + 1 + screen.url_input.len() as u16,
            inner[1].y + 1,
        );
    } else if is_saving {
        f.set_cursor(
            inner[2].x + 1 + screen.name_input.len() as u16,
            inner[2].y + 1,
        );
    }

    // Status / hint
    let hint_text = if let Some(ref msg) = screen.status {
        msg.as_str()
    } else if is_saving {
        "Enter: save profile   Esc: back to URL"
    } else if is_editing {
        "Enter: connect   Ctrl+S: save   Esc: cancel   Tab: type"
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
        inner[3],
    );
}

fn draw_help(f: &mut Frame<'_>, screen: &ConnectionScreen, area: Rect) {
    let confirm_text;
    let text: &str = match screen.input_mode {
        InputMode::Normal =>
            " j/k: move   Enter: connect   n: new   D: delete profile   q: quit ",
        InputMode::Editing =>
            " Esc: cancel   Tab: type   Enter: connect   Ctrl+S: save profile ",
        InputMode::SavingName =>
            " Type a name for this connection   Enter: save   Esc: back ",
        InputMode::ConfirmDelete => {
            let name = screen.pending_delete
                .and_then(|i| screen.profiles.get(i))
                .map(|p| p.name.as_str())
                .unwrap_or("?");
            confirm_text = format!(
                " Delete \"{}\"?   y: delete from file   n: remove from list only   Esc: cancel ",
                name
            );
            &confirm_text
        }
    };
    let style = if matches!(screen.input_mode, InputMode::ConfirmDelete) {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .style(style),
        area,
    );
}
