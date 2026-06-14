use std::collections::HashSet;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row as RatRow, Table, TableState},
    Frame,
};
use crate::db::types::{DbQueryResult, Value};

const COLLAPSED_WIDTH: u16 = 3;
const MAX_COL_WIDTH: u16 = 25;

pub enum DataGridAction {
    None,
    Back,
}

pub struct DataGridScreen {
    pub table_name: String,
    pub result: Option<DbQueryResult>,
    pub table_state: TableState,
    pub selected_col: usize,
    pub col_offset: usize,
    pub collapsed_cols: HashSet<usize>,
    pub status: Option<String>,
}

impl DataGridScreen {
    pub fn new(table_name: String) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            table_name,
            result: None,
            table_state,
            selected_col: 0,
            col_offset: 0,
            collapsed_cols: HashSet::new(),
            status: Some("Loading…".into()),
        }
    }

    pub fn set_result(&mut self, result: DbQueryResult) {
        self.status = None;
        self.selected_col = 0;
        self.col_offset = 0;
        self.collapsed_cols.clear();
        self.result = Some(result);
        self.table_state.select(Some(0));
    }

    pub fn set_error(&mut self, msg: String) {
        self.status = Some(format!("Error: {msg}"));
    }

    fn row_count(&self) -> usize {
        self.result.as_ref().map_or(0, |r| r.rows.len())
    }

    fn col_count(&self) -> usize {
        self.result.as_ref().map_or(0, |r| r.columns.len())
    }

    fn selected_row(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    fn effective_col_width(&self, col_idx: usize) -> u16 {
        if self.collapsed_cols.contains(&col_idx) {
            return COLLAPSED_WIDTH;
        }
        let Some(ref result) = self.result else { return 10; };
        let header_w = result.columns[col_idx].name.len() as u16;
        let max_val_w = result
            .rows
            .iter()
            .map(|r| value_display(r.values.get(col_idx).unwrap_or(&Value::Null)).len() as u16)
            .max()
            .unwrap_or(0);
        (header_w.max(max_val_w) + 2).min(MAX_COL_WIDTH)
    }

    fn visible_columns(&self, available_width: u16) -> Vec<usize> {
        let col_count = self.col_count();
        let mut visible = vec![];
        let mut used = 1u16; // left border
        for i in self.col_offset..col_count {
            let w = self.effective_col_width(i);
            used += w + 1; // +1 for column separator
            if used > available_width {
                break;
            }
            visible.push(i);
        }
        visible
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> DataGridAction {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc      => DataGridAction::Back,
            KeyCode::Char('j') | KeyCode::Down     => { self.move_row(1);   DataGridAction::None }
            KeyCode::Char('k') | KeyCode::Up       => { self.move_row(-1);  DataGridAction::None }
            KeyCode::Char('l') | KeyCode::Right    => { self.move_col(1);   DataGridAction::None }
            KeyCode::Char('h') | KeyCode::Left     => { self.move_col(-1);  DataGridAction::None }
            KeyCode::Char('g')                     => { self.go_first();    DataGridAction::None }
            KeyCode::Char('G')                     => { self.go_last();     DataGridAction::None }
            KeyCode::PageDown                      => { self.move_row(10);  DataGridAction::None }
            KeyCode::PageUp                        => { self.move_row(-10); DataGridAction::None }
            KeyCode::Char(' ')                     => { self.toggle_collapse(); DataGridAction::None }
            _ => DataGridAction::None,
        }
    }

    fn move_row(&mut self, delta: i64) {
        let count = self.row_count();
        if count == 0 { return; }
        let next = (self.selected_row() as i64 + delta).clamp(0, count as i64 - 1) as usize;
        self.table_state.select(Some(next));
    }

    fn move_col(&mut self, delta: i64) {
        let count = self.col_count();
        if count == 0 { return; }
        let next = (self.selected_col as i64 + delta).clamp(0, count as i64 - 1) as usize;
        self.selected_col = next;
        // Scroll left: keep col_offset ≤ selected_col
        if next < self.col_offset {
            self.col_offset = next;
        }
        // Scroll right: handled in draw() once we know terminal width
    }

    fn go_first(&mut self) {
        if self.row_count() > 0 {
            self.table_state.select(Some(0));
        }
    }

    fn go_last(&mut self) {
        let count = self.row_count();
        if count > 0 {
            self.table_state.select(Some(count - 1));
        }
    }

    fn toggle_collapse(&mut self) {
        if self.col_count() == 0 { return; }
        if self.collapsed_cols.contains(&self.selected_col) {
            self.collapsed_cols.remove(&self.selected_col);
        } else {
            self.collapsed_cols.insert(self.selected_col);
        }
    }

    pub fn draw(f: &mut Frame<'_>, screen: &mut DataGridScreen) {
        let area = f.size();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // info bar
                Constraint::Min(0),    // data table
                Constraint::Length(3), // help bar
            ])
            .split(area);

        // ── Info bar ──────────────────────────────────────────────────────────
        let info = if let Some(ref r) = screen.result {
            let row_info = if r.rows.is_empty() {
                "no rows".to_string()
            } else {
                format!("row {}/{}", screen.selected_row() + 1, r.rows.len())
            };
            format!(
                " {} │ {} │ col {}/{}  (LIMIT 1000) ",
                screen.table_name,
                row_info,
                screen.selected_col + 1,
                r.columns.len(),
            )
        } else {
            format!(" {} ", screen.table_name)
        };
        f.render_widget(
            Paragraph::new(info)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            chunks[0],
        );

        // ── Data table ────────────────────────────────────────────────────────

        // Adjust col_offset so selected_col is always in view
        if screen.selected_col < screen.col_offset {
            screen.col_offset = screen.selected_col;
        }
        loop {
            let visible = screen.visible_columns(chunks[1].width);
            if visible.is_empty() || *visible.last().unwrap() >= screen.selected_col {
                break;
            }
            screen.col_offset += 1;
        }

        let visible = screen.visible_columns(chunks[1].width);

        match &screen.result {
            None => {
                let msg = screen.status.clone().unwrap_or_else(|| "No data".into());
                f.render_widget(
                    Paragraph::new(msg)
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
            }
            Some(result) if result.rows.is_empty() => {
                f.render_widget(
                    Paragraph::new("Table is empty")
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
            }
            Some(result) => {
                // Owned copies so we can borrow screen mutably for table_state below
                let widths: Vec<Constraint> = visible
                    .iter()
                    .map(|&i| Constraint::Length(screen.effective_col_width(i)))
                    .collect();

                let header_cells: Vec<Cell> = visible
                    .iter()
                    .map(|&i| {
                        let name = if screen.collapsed_cols.contains(&i) {
                            "…".to_string()
                        } else {
                            truncate(&result.columns[i].name, screen.effective_col_width(i) as usize)
                        };
                        let style = if i == screen.selected_col {
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                        } else {
                            Style::default().add_modifier(Modifier::BOLD)
                        };
                        Cell::from(name).style(style)
                    })
                    .collect();

                let header = RatRow::new(header_cells)
                    .style(Style::default().bg(Color::DarkGray))
                    .height(1);

                let data_rows: Vec<RatRow> = result
                    .rows
                    .iter()
                    .map(|row| {
                        let cells: Vec<Cell> = visible
                            .iter()
                            .map(|&i| {
                                let val = row.values.get(i).unwrap_or(&Value::Null);
                                let s = value_display(val);
                                let display = if screen.collapsed_cols.contains(&i) {
                                    s.chars().next()
                                        .map(|c| format!("{c}…"))
                                        .unwrap_or_else(|| "…".into())
                                } else {
                                    truncate(&s, screen.effective_col_width(i) as usize)
                                };
                                Cell::from(display)
                            })
                            .collect();
                        RatRow::new(cells).height(1)
                    })
                    .collect();

                let table = Table::new(data_rows, widths)
                    .header(header)
                    .block(Block::default().borders(Borders::ALL))
                    .highlight_style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");

                f.render_stateful_widget(table, chunks[1], &mut screen.table_state);
            }
        }

        // ── Help bar ──────────────────────────────────────────────────────────
        let collapse_label = if screen.collapsed_cols.contains(&screen.selected_col) {
            "Space: expand"
        } else {
            "Space: collapse"
        };
        f.render_widget(
            Paragraph::new(format!(
                " j/k: rows   h/l: cols   g/G: first/last   PgUp/Dn: page   {}   q: back ",
                collapse_label
            ))
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray)),
            chunks[2],
        );
    }
}

fn value_display(v: &Value) -> String {
    match v {
        Value::Null     => "NULL".into(),
        Value::Bool(b)  => b.to_string(),
        Value::Int(i)   => i.to_string(),
        Value::Float(f) => format!("{f:.4}"),
        Value::Text(s)  => s.replace('\n', "↵").replace('\r', ""),
        Value::Bytes(b) => format!("<{} bytes>", b.len()),
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    let max = max_chars.saturating_sub(1); // leave room for possible '…'
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
