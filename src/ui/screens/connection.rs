use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use crate::config::ConnectionProfile;

const DB_TYPES: &[&str] = &["postgres", "sqlite", "libsql", "mysql", "redis", "mongodb", "duckdb"];

pub enum InputMode {
    Normal,
    Editing,
    SavingName,
    ConfirmDelete,
}

#[derive(Default, PartialEq)]
pub enum EditField {
    #[default]
    DbType,
    Url,
    PreConnect,
    PostDisconnect,
}

pub struct ConnectionScreen {
    pub profiles: Vec<ConnectionProfile>,
    pub list_state: ListState,
    pub input_mode: InputMode,
    pub url_input: String,
    pub name_input: String,
    pub pre_connect_input: String,
    pub post_disconnect_input: String,
    pub db_type_idx: usize,
    pub focused_field: EditField,
    pub editing_profile_name: Option<String>,
    pub status: Option<String>,
    pub pending_delete: Option<usize>,
}

pub enum ConnectionAction {
    None,
    Connect { url: String, db_type: String, pre_connect: Option<String>, post_disconnect: Option<String>, profile_name: Option<String> },
    SaveProfile { name: String, url: String, db_type: String, pre_connect: Option<String>, post_disconnect: Option<String> },
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
            pre_connect_input: String::new(),
            post_disconnect_input: String::new(),
            db_type_idx: 0,
            focused_field: EditField::Url,
            editing_profile_name: None,
            status: None,
            pending_delete: None,
        }
    }

    pub fn reset_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.url_input.clear();
        self.name_input.clear();
        self.pre_connect_input.clear();
        self.post_disconnect_input.clear();
        self.focused_field = EditField::Url;
        self.editing_profile_name = None;
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
                self.pre_connect_input.clear();
                self.post_disconnect_input.clear();
                self.focused_field = EditField::Url;
                self.editing_profile_name = None;
                self.status = None;
                ConnectionAction::None
            }
            KeyCode::Char('e') => {
                if let Some(p) = self.selected_profile().cloned() {
                    self.db_type_idx = DB_TYPES.iter().position(|&t| t == p.db_type).unwrap_or(0);
                    self.url_input = p.url;
                    self.pre_connect_input = p.pre_connect.unwrap_or_default();
                    self.post_disconnect_input = p.post_disconnect.unwrap_or_default();
                    self.editing_profile_name = Some(p.name.clone());
                    self.name_input = p.name;
                    self.focused_field = EditField::Url;
                    self.input_mode = InputMode::Editing;
                    self.status = None;
                }
                ConnectionAction::None
            }
            KeyCode::Char('D') | KeyCode::Delete => {
                if let Some(i) = self.list_state.selected()
                    && i < self.profiles.len() {
                        self.pending_delete = Some(i);
                        self.input_mode = InputMode::ConfirmDelete;
                        self.status = None;
                    }
                ConnectionAction::None
            }
            KeyCode::Enter => {
                if let Some(p) = self.selected_profile() {
                    ConnectionAction::Connect {
                        url: p.url.clone(),
                        db_type: p.db_type.clone(),
                        pre_connect: p.pre_connect.clone(),
                        post_disconnect: p.post_disconnect.clone(),
                        profile_name: Some(p.name.clone()),
                    }
                } else {
                    ConnectionAction::None
                }
            }
            _ => ConnectionAction::None,
        }
    }

    fn handle_editing(&mut self, key: KeyEvent) -> ConnectionAction {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('s') {
                if self.url_input.is_empty() {
                    self.status = Some("Enter a URL first".into());
                } else {
                    self.input_mode = InputMode::SavingName;
                    self.status = None;
                }
                return ConnectionAction::None;
            }
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                ConnectionAction::None
            }
            KeyCode::Tab => {
                self.focused_field = match self.focused_field {
                    EditField::DbType      => EditField::Url,
                    EditField::Url         => EditField::PreConnect,
                    EditField::PreConnect  => EditField::PostDisconnect,
                    EditField::PostDisconnect => EditField::DbType,
                };
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
                    pre_connect: nonempty(self.pre_connect_input.trim()),
                    post_disconnect: nonempty(self.post_disconnect_input.trim()),
                    profile_name: None,
                }
            }
            KeyCode::Left | KeyCode::Right if self.focused_field == EditField::DbType => {
                self.db_type_idx = (self.db_type_idx + 1) % DB_TYPES.len();
                ConnectionAction::None
            }
            KeyCode::Backspace => {
                match self.focused_field {
                    EditField::DbType         => {}
                    EditField::Url            => { self.url_input.pop(); }
                    EditField::PreConnect     => { self.pre_connect_input.pop(); }
                    EditField::PostDisconnect => { self.post_disconnect_input.pop(); }
                }
                ConnectionAction::None
            }
            KeyCode::Char(c) => {
                match self.focused_field {
                    EditField::DbType         => { self.db_type_idx = (self.db_type_idx + 1) % DB_TYPES.len(); }
                    EditField::Url            => { self.url_input.push(c); }
                    EditField::PreConnect     => { self.pre_connect_input.push(c); }
                    EditField::PostDisconnect => { self.post_disconnect_input.push(c); }
                }
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
                    pre_connect: nonempty(self.pre_connect_input.trim()),
                    post_disconnect: nonempty(self.post_disconnect_input.trim()),
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

    pub fn draw(f: &mut Frame<'_>, screen: &mut ConnectionScreen, area: Rect) {

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

    let panel_title = if let Some(ref name) = screen.editing_profile_name {
        format!(" Edit: {name} ")
    } else {
        " New Connection ".to_string()
    };
    f.render_widget(
        Block::default()
            .title(panel_title)
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
            Constraint::Length(3), // Pre-connect
            Constraint::Length(3), // Post-disconnect
            Constraint::Length(3), // Name (for save)
            Constraint::Min(0),    // Hint
        ])
        .split(area);

    let active = |field: &EditField| is_editing && &screen.focused_field == field;

    // DB type selector
    let type_text = format!(" < {} >  (Tab / ← → to cycle)", screen.current_db_type());
    let type_style = if active(&EditField::DbType) {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let type_block = Block::default()
        .title(" Type ")
        .borders(Borders::ALL)
        .border_style(if active(&EditField::DbType) { Style::default().fg(Color::Yellow) } else { Style::default() });
    f.render_widget(Paragraph::new(type_text).block(type_block).style(type_style), inner[0]);

    // URL input
    let url_display = if screen.url_input.is_empty() && !is_editing && !is_saving {
        "Press 'n' to enter a URL, or 'e' to edit selected profile…".to_string()
    } else {
        screen.url_input.clone()
    };
    let url_focused = active(&EditField::Url) || is_saving;
    let url_style = if url_focused { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Gray) };
    let url_block = Block::default()
        .title(" URL ")
        .borders(Borders::ALL)
        .border_style(if active(&EditField::Url) { Style::default().fg(Color::Yellow) } else { Style::default() });
    f.render_widget(Paragraph::new(url_display).block(url_block).style(url_style), inner[1]);

    // Pre-connect script
    let pre_display = if screen.pre_connect_input.is_empty() && !is_editing {
        String::new()
    } else {
        screen.pre_connect_input.clone()
    };
    let pre_focused = active(&EditField::PreConnect);
    let pre_style = if pre_focused { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Gray) };
    let pre_block = Block::default()
        .title(" Pre-connect script (optional) ")
        .borders(Borders::ALL)
        .border_style(if pre_focused { Style::default().fg(Color::Yellow) } else { Style::default() });
    f.render_widget(Paragraph::new(pre_display).block(pre_block).style(pre_style), inner[2]);

    // Post-disconnect script
    let post_display = if screen.post_disconnect_input.is_empty() && !is_editing {
        String::new()
    } else {
        screen.post_disconnect_input.clone()
    };
    let post_focused = active(&EditField::PostDisconnect);
    let post_style = if post_focused { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Gray) };
    let post_block = Block::default()
        .title(" Post-disconnect script (optional) ")
        .borders(Borders::ALL)
        .border_style(if post_focused { Style::default().fg(Color::Yellow) } else { Style::default() });
    f.render_widget(Paragraph::new(post_display).block(post_block).style(post_style), inner[3]);

    // Name input (visible when saving)
    let name_display = if screen.name_input.is_empty() && !is_saving {
        "Ctrl+S to save with a name…".to_string()
    } else {
        screen.name_input.clone()
    };
    let name_style = if is_saving { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Gray) };
    let name_block = Block::default()
        .title(" Save as (name) ")
        .borders(Borders::ALL)
        .border_style(if is_saving { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::Gray) });
    f.render_widget(Paragraph::new(name_display).block(name_block).style(name_style), inner[4]);

    // Cursor positioning
    if is_editing {
        match screen.focused_field {
            EditField::DbType         => {}
            EditField::Url            => { f.set_cursor(inner[1].x + 1 + screen.url_input.len() as u16, inner[1].y + 1); }
            EditField::PreConnect     => { f.set_cursor(inner[2].x + 1 + screen.pre_connect_input.len() as u16, inner[2].y + 1); }
            EditField::PostDisconnect => { f.set_cursor(inner[3].x + 1 + screen.post_disconnect_input.len() as u16, inner[3].y + 1); }
        }
    } else if is_saving {
        f.set_cursor(inner[4].x + 1 + screen.name_input.len() as u16, inner[4].y + 1);
    }

    // Status / hint
    let hint_text = if let Some(ref msg) = screen.status {
        msg.as_str()
    } else if is_saving {
        "Enter: save profile   Esc: back"
    } else if is_editing {
        "Tab: next field   ← →: type   Enter: connect   Ctrl+S: save   Esc: cancel"
    } else {
        "'n' to enter a new connection"
    };
    let hint_style = if screen.status.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Gray)
    };
    f.render_widget(
        Paragraph::new(hint_text).style(hint_style).wrap(Wrap { trim: false }),
        inner[5],
    );
}

fn nonempty(s: &str) -> Option<String> {
    if s.is_empty() { None } else { Some(s.to_string()) }
}

fn draw_help(f: &mut Frame<'_>, screen: &ConnectionScreen, area: Rect) {
    let confirm_text;
    let text: &str = match screen.input_mode {
        InputMode::Normal =>
            " j/k: move   Enter: connect   n: new   e: edit   D: delete   Ctrl+T: new tab   [/]: prev/next tab   q: quit ",
        InputMode::Editing =>
            " Tab: field   ← →: type   Enter: connect   Ctrl+S: save   Esc: cancel ",
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
        Style::default().fg(Color::Gray)
    };
    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .style(style),
        area,
    );
}
