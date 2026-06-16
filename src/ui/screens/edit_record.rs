use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::db::types::ColumnSchema;

const NAME_W: usize = 20;
const BADGE_W: usize = 14;
const TYPE_W: usize = 16;

// ── Modes ─────────────────────────────────────────────────────────────────────

pub enum EditFieldMode {
    Navigate,
    Editing,
}

// ── Actions ───────────────────────────────────────────────────────────────────

pub enum EditRecordAction {
    None,
    Back,
    Save(String),
}

// ── Screen ────────────────────────────────────────────────────────────────────

pub struct EditRecordScreen {
    pub table_name: String,
    pub schema: Vec<ColumnSchema>,
    pub original_values: Vec<String>,
    pub current_values: Vec<String>,
    pub selected_field: usize,
    pub cursor_pos: usize,
    pub mode: EditFieldMode,
    pub scroll_offset: usize,
    pub status: Option<String>,
    pub validation_errors: Vec<Option<String>>,
}

impl EditRecordScreen {
    pub fn new(table_name: String, schema: Vec<ColumnSchema>, values: Vec<String>) -> Self {
        let n = values.len();
        Self {
            table_name,
            original_values: values.clone(),
            current_values: values,
            schema,
            selected_field: 0,
            cursor_pos: 0,
            mode: EditFieldMode::Navigate,
            scroll_offset: 0,
            status: None,
            validation_errors: vec![None; n],
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.status = Some(format!("Error: {msg}"));
        self.mode = EditFieldMode::Navigate;
    }

    // ── Key handling ──────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: KeyEvent) -> EditRecordAction {
        match self.mode {
            EditFieldMode::Navigate => self.handle_navigate(key),
            EditFieldMode::Editing  => self.handle_edit(key),
        }
    }

    fn handle_navigate(&mut self, key: KeyEvent) -> EditRecordAction {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => EditRecordAction::Back,

            KeyCode::Char('j') | KeyCode::Down => {
                self.move_field(1);
                EditRecordAction::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_field(-1);
                EditRecordAction::None
            }

            KeyCode::Enter | KeyCode::Char('i') => {
                if let Some(col) = self.schema.get(self.selected_field) {
                    if !col.is_pk {
                        self.cursor_pos = self.current_values[self.selected_field].chars().count();
                        self.mode = EditFieldMode::Editing;
                        self.status = None;
                    }
                }
                EditRecordAction::None
            }

            // Toggle boolean with Space
            KeyCode::Char(' ') => {
                if let Some(col) = self.schema.get(self.selected_field) {
                    if !col.is_pk && is_bool_type(&col.type_name) {
                        let val = &mut self.current_values[self.selected_field];
                        *val = if is_truthy(val) { "false".to_string() } else { "true".to_string() };
                    }
                }
                EditRecordAction::None
            }

            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let error_count = self.validation_errors.iter().filter(|e| e.is_some()).count();
                if error_count > 0 {
                    self.status = Some(format!("{error_count} field(s) with invalid format"));
                    return EditRecordAction::None;
                }
                let sql = self.build_update_sql();
                if sql.starts_with("-- ") {
                    self.status = Some(sql);
                    EditRecordAction::None
                } else {
                    EditRecordAction::Save(sql)
                }
            }

            _ => EditRecordAction::None,
        }
    }

    fn handle_edit(&mut self, key: KeyEvent) -> EditRecordAction {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                if let Some(col) = self.schema.get(self.selected_field) {
                    let val = &self.current_values[self.selected_field];
                    self.validation_errors[self.selected_field] = validate_field(&col.type_name, val);
                }
                self.mode = EditFieldMode::Navigate;
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Right => {
                let len = self.current_values[self.selected_field].chars().count();
                if self.cursor_pos < len {
                    self.cursor_pos += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.current_values[self.selected_field].chars().count();
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    let val = &mut self.current_values[self.selected_field];
                    let mut chars: Vec<char> = val.chars().collect();
                    chars.remove(self.cursor_pos - 1);
                    *val = chars.into_iter().collect();
                    self.cursor_pos -= 1;
                }
            }
            KeyCode::Delete => {
                let val = &mut self.current_values[self.selected_field];
                let len = val.chars().count();
                if self.cursor_pos < len {
                    let mut chars: Vec<char> = val.chars().collect();
                    chars.remove(self.cursor_pos);
                    *val = chars.into_iter().collect();
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let val = &mut self.current_values[self.selected_field];
                let mut chars: Vec<char> = val.chars().collect();
                chars.insert(self.cursor_pos, c);
                *val = chars.into_iter().collect();
                self.cursor_pos += 1;
            }
            _ => {}
        }
        EditRecordAction::None
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn move_field(&mut self, delta: i64) {
        let count = self.schema.len();
        if count == 0 { return; }
        let next = (self.selected_field as i64 + delta).clamp(0, count as i64 - 1) as usize;
        self.selected_field = next;
    }

    pub fn build_update_sql(&self) -> String {
        let Some(pk_idx) = self.schema.iter().position(|c| c.is_pk) else {
            return "-- No primary key: cannot generate UPDATE".to_string();
        };

        let pk_col = &self.schema[pk_idx].name;
        let pk_orig = &self.original_values[pk_idx];

        let changes: Vec<String> = self.schema.iter()
            .zip(self.current_values.iter())
            .zip(self.original_values.iter())
            .filter(|((col, cur), orig)| !col.is_pk && *cur != *orig)
            .map(|((col, cur), _)| {
                format!("\"{}\" = {}", col.name, sql_literal(cur, &col.type_name))
            })
            .collect();

        if changes.is_empty() {
            return "-- No changes".to_string();
        }

        format!(
            "UPDATE \"{}\" SET {} WHERE \"{}\" = {}",
            self.table_name,
            changes.join(", "),
            pk_col,
            sql_literal(pk_orig, &self.schema[pk_idx].type_name)
        )
    }

    // ── Draw ──────────────────────────────────────────────────────────────────

    pub fn draw(f: &mut Frame<'_>, screen: &mut EditRecordScreen, area: Rect) {

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // fields
                Constraint::Length(4), // SQL preview
                Constraint::Length(3), // help bar
            ])
            .split(area);

        // ── Adjust scroll offset ──────────────────────────────────────────────
        let visible_rows = (chunks[0].height as usize).saturating_sub(2);
        if screen.selected_field < screen.scroll_offset {
            screen.scroll_offset = screen.selected_field;
        } else if screen.selected_field >= screen.scroll_offset + visible_rows && visible_rows > 0 {
            screen.scroll_offset = screen.selected_field - visible_rows + 1;
        }

        let val_w = (chunks[0].width as usize).saturating_sub(2 + NAME_W + BADGE_W + TYPE_W);

        // ── Build field lines ─────────────────────────────────────────────────
        let mut lines: Vec<Line> = vec![];
        let end = (screen.scroll_offset + visible_rows).min(screen.schema.len());

        for i in screen.scroll_offset..end {
            let col = &screen.schema[i];
            let is_sel = i == screen.selected_field;
            let is_editing = is_sel && matches!(screen.mode, EditFieldMode::Editing);
            let cur_val = &screen.current_values[i];
            let orig_val = &screen.original_values[i];
            let changed = cur_val != orig_val && !col.is_pk;
            let has_error = screen.validation_errors.get(i).and_then(|e| e.as_ref()).is_some();

            let sel_str = if is_sel { "> " } else { "  " };
            let name_str = format!("{:<width$}", col.name, width = NAME_W - 2);

            let (badge_str, badge_style) = if col.is_pk {
                (
                    format!("{:<BADGE_W$}", "[PK]"),
                    Style::default().fg(Color::Cyan),
                )
            } else if let Some(ref fk) = col.fk {
                let max_t = BADGE_W.saturating_sub(4);
                let t = if fk.table.len() > max_t { &fk.table[..max_t] } else { &fk.table };
                (
                    format!("[→{:<width$}]", t, width = BADGE_W - 3),
                    Style::default().fg(Color::Magenta),
                )
            } else {
                (
                    format!("{:<BADGE_W$}", ""),
                    Style::default(),
                )
            };

            let display_val = if cur_val.chars().count() > val_w && val_w > 0 {
                let s: String = cur_val.chars().take(val_w.saturating_sub(1)).collect();
                format!("{s}…")
            } else {
                cur_val.clone()
            };

            let name_style = if col.is_pk {
                Style::default().fg(Color::DarkGray)
            } else if is_sel {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if changed {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            let val_style = if col.is_pk {
                Style::default().fg(Color::DarkGray)
            } else if has_error && !is_editing {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if is_editing {
                Style::default().fg(Color::White).bg(Color::DarkGray)
            } else if is_sel {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else if cur_val == "NULL" {
                Style::default().fg(Color::DarkGray)
            } else if changed {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            };

            let raw_type = &col.type_name;
            let type_str = format!(
                "{:<TYPE_W$}",
                if raw_type.chars().count() > TYPE_W {
                    raw_type.chars().take(TYPE_W - 1).collect::<String>() + "…"
                } else {
                    raw_type.clone()
                }
            );
            let type_style = Style::default().fg(Color::Blue);

            lines.push(Line::from(vec![
                Span::styled(format!("{sel_str}{name_str}"), name_style),
                Span::styled(badge_str, badge_style),
                Span::styled(type_str, type_style),
                Span::styled(display_val, val_style),
            ]));
        }

        let title_suffix = screen.status.as_deref()
            .map(|s| format!(" — {s}"))
            .unwrap_or_default();
        let title = format!(" Edit: {}{} ", screen.table_name, title_suffix);

        f.render_widget(
            Paragraph::new(Text::from(lines))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ),
            chunks[0],
        );

        // ── Terminal cursor when editing ──────────────────────────────────────
        if matches!(screen.mode, EditFieldMode::Editing) {
            let field_row = screen.selected_field.saturating_sub(screen.scroll_offset);
            let visible_cursor = screen.cursor_pos.min(val_w);
            let cursor_x = chunks[0].x + 1 + (NAME_W + BADGE_W + TYPE_W) as u16 + visible_cursor as u16;
            let cursor_y = chunks[0].y + 1 + field_row as u16;
            if cursor_y < chunks[0].y + chunks[0].height.saturating_sub(1) {
                f.set_cursor(cursor_x, cursor_y);
            }
        }

        // ── SQL preview ───────────────────────────────────────────────────────
        let sql = screen.build_update_sql();
        let sql_style = if sql.starts_with("-- ") {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Green)
        };
        f.render_widget(
            Paragraph::new(sql.as_str())
                .block(Block::default().borders(Borders::ALL).title(" SQL Preview "))
                .style(sql_style)
                .wrap(Wrap { trim: false }),
            chunks[1],
        );

        // ── Help bar ──────────────────────────────────────────────────────────
        let (help_text, help_style) = if matches!(screen.mode, EditFieldMode::Editing) {
            let hint = screen.schema.get(screen.selected_field)
                .and_then(|col| format_hint(&col.type_name))
                .map(|h| format!(" Format: {h}   ← →: cursor   Backspace/Del   Enter/Esc: done"))
                .unwrap_or_else(|| " ← →: cursor   Backspace/Del: delete   Enter/Esc: done".into());
            (hint, Style::default().fg(Color::Cyan))
        } else if let Some(err) = screen.schema.get(screen.selected_field)
            .and_then(|_| screen.validation_errors.get(screen.selected_field))
            .and_then(|e| e.as_ref())
        {
            (format!(" ✗ {err}"), Style::default().fg(Color::Red))
        } else {
            (
                " j/k: field   Enter/i: edit   Space: toggle bool   Ctrl+S: save   Esc: back".into(),
                Style::default().fg(Color::DarkGray),
            )
        };
        f.render_widget(
            Paragraph::new(help_text.as_str())
                .block(Block::default().borders(Borders::ALL))
                .style(help_style),
            chunks[2],
        );
    }
}

// ── Format validation ─────────────────────────────────────────────────────────

fn validate_field(type_name: &str, value: &str) -> Option<String> {
    if value.is_empty() || value == "NULL" { return None; }
    let tn = type_name.to_uppercase();

    if (tn.contains("DATE") && !tn.contains("DATETIME") && !tn.contains("TIMESTAMP"))
        || tn == "DATE"
    {
        if NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err() {
            return Some("expected YYYY-MM-DD".into());
        }
    } else if tn == "TIME" {
        if NaiveTime::parse_from_str(value, "%H:%M:%S").is_err()
            && NaiveTime::parse_from_str(value, "%H:%M").is_err()
        {
            return Some("expected HH:MM:SS".into());
        }
    } else if tn.contains("TIMESTAMP") || tn.contains("DATETIME") {
        let ok = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").is_ok()
            || NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S").is_ok()
            || chrono::DateTime::parse_from_rfc3339(value).is_ok();
        if !ok {
            return Some("expected YYYY-MM-DD HH:MM:SS".into());
        }
    } else if tn.contains("UUID") {
        if !is_valid_uuid(value) {
            return Some("expected xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx".into());
        }
    } else if tn.contains("JSON") {
        if serde_json::from_str::<serde_json::Value>(value).is_err() {
            return Some("invalid JSON".into());
        }
    } else if (tn.contains("INT") || tn == "BIGINT" || tn == "SMALLINT")
        && !tn.contains("INTERVAL")
    {
        if value.parse::<i64>().is_err() {
            return Some("expected integer".into());
        }
    } else if tn.contains("FLOAT") || tn.contains("REAL") || tn.contains("DOUBLE")
        || tn.contains("NUMERIC") || tn.contains("DECIMAL")
    {
        if value.parse::<f64>().is_err() {
            return Some("expected number".into());
        }
    } else if tn.contains("INET") || tn.contains("CIDR") {
        let host = value.splitn(2, '/').next().unwrap_or(value);
        if host.parse::<std::net::IpAddr>().is_err() {
            return Some("expected IP address".into());
        }
    }
    None
}

fn format_hint(type_name: &str) -> Option<&'static str> {
    let tn = type_name.to_uppercase();
    if (tn.contains("DATE") && !tn.contains("DATETIME") && !tn.contains("TIMESTAMP"))
        || tn == "DATE"
    {
        Some("YYYY-MM-DD")
    } else if tn == "TIME" {
        Some("HH:MM:SS")
    } else if tn.contains("TIMESTAMP") || tn.contains("DATETIME") {
        Some("YYYY-MM-DD HH:MM:SS")
    } else if tn.contains("UUID") {
        Some("xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx")
    } else if tn.contains("JSON") {
        Some("valid JSON")
    } else if tn.contains("INET") || tn.contains("CIDR") {
        Some("IP address  e.g. 192.168.1.1")
    } else {
        None
    }
}

fn is_valid_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 { return false; }
    let lens = [8, 4, 4, 4, 12];
    parts.iter().zip(lens.iter()).all(|(p, &l)| {
        p.len() == l && p.chars().all(|c| c.is_ascii_hexdigit())
    })
}

// ── SQL helpers ───────────────────────────────────────────────────────────────

fn is_bool_type(type_name: &str) -> bool {
    let tn = type_name.to_uppercase();
    tn.contains("BOOL") || tn == "TINYINT(1)"
}

fn is_truthy(val: &str) -> bool {
    matches!(val.to_lowercase().as_str(), "true" | "t" | "1" | "yes" | "on")
}

fn sql_literal(val: &str, type_name: &str) -> String {
    if val == "NULL" {
        return "NULL".to_string();
    }
    let tn = type_name.to_uppercase();
    if is_bool_type(type_name) {
        return if is_truthy(val) { "TRUE".to_string() } else { "FALSE".to_string() };
    }
    let is_num = tn.contains("INT") || tn.contains("FLOAT") || tn.contains("REAL")
        || tn.contains("NUMERIC") || tn.contains("DECIMAL") || tn.contains("DOUBLE")
        || tn.contains("NUMBER");
    if is_num && val.parse::<f64>().is_ok() {
        val.to_string()
    } else {
        format!("'{}'", val.replace('\'', "''"))
    }
}
