use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use crate::db::types::ColumnSchema;

const NAME_W: usize = 20;
const BADGE_W: usize = 14;

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
}

impl EditRecordScreen {
    pub fn new(table_name: String, schema: Vec<ColumnSchema>, values: Vec<String>) -> Self {
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

            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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

    pub fn draw(f: &mut Frame<'_>, screen: &mut EditRecordScreen) {
        let area = f.size();

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

        let val_w = (chunks[0].width as usize).saturating_sub(2 + NAME_W + BADGE_W);

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

            lines.push(Line::from(vec![
                Span::styled(format!("{sel_str}{name_str}"), name_style),
                Span::styled(badge_str, badge_style),
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
            let cursor_x = chunks[0].x + 1 + (NAME_W + BADGE_W) as u16 + visible_cursor as u16;
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
        let help = if matches!(screen.mode, EditFieldMode::Editing) {
            " ← →: cursor   Backspace/Del: delete   Enter/Esc: done"
        } else {
            " j/k: field   Enter/i: edit   Ctrl+S: save   Esc: back"
        };
        f.render_widget(
            Paragraph::new(help)
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray)),
            chunks[2],
        );
    }
}

// ── SQL helpers ───────────────────────────────────────────────────────────────

fn sql_literal(val: &str, type_name: &str) -> String {
    if val == "NULL" {
        return "NULL".to_string();
    }
    let tn = type_name.to_uppercase();
    let is_num = tn.contains("INT") || tn.contains("FLOAT") || tn.contains("REAL")
        || tn.contains("NUMERIC") || tn.contains("DECIMAL") || tn.contains("DOUBLE")
        || tn.contains("NUMBER");
    if is_num && val.parse::<f64>().is_ok() {
        val.to_string()
    } else {
        format!("'{}'", val.replace('\'', "''"))
    }
}
