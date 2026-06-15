use std::collections::{BTreeMap, HashMap, HashSet};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row as RatRow, Table, TableState},
    Frame,
};
use crate::db::types::{ColumnSchema, DbQueryResult, Value};

pub const PAGE_SIZE: usize = 200;
const COLLAPSED_WIDTH: u16 = 3;
const MAX_COL_WIDTH: u16 = 40;
const COL_RESIZE_STEP: u16 = 5;

// ── Filter input (in-progress edit) ──────────────────────────────────────────

pub struct FilterInput {
    pub col_name: String,
    pub value: String,
}

// ── Actions ───────────────────────────────────────────────────────────────────

pub enum DataGridAction {
    None,
    Back,
    ApplyFilter,
    LoadMore,
    EnterCell,
    ExportCsv,
    ExportJson,
}

// ── Screen ────────────────────────────────────────────────────────────────────

pub struct DataGridScreen {
    pub table_name: String,
    pub display_name: Option<String>, // label shown in info bar (e.g. "books [id=1]")
    pub result: Option<DbQueryResult>,
    pub schema: Option<Vec<ColumnSchema>>,
    pub table_state: TableState,
    pub selected_col: usize,
    pub col_offset: usize,
    pub collapsed_cols: HashSet<usize>,
    pub col_widths: HashMap<usize, u16>,
    pub read_only: bool,
    pub status: Option<String>,
    pub export_prompt: bool,
    // Filtering
    pub filters: BTreeMap<String, String>,
    pub filter_input: Option<FilterInput>,
    // Pagination
    pub loaded_count: usize,
    pub total_count: Option<u64>,
    pub has_more: bool,
    pub loading: bool,
}

impl DataGridScreen {
    pub fn new(table_name: String) -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            table_name,
            display_name: None,
            result: None,
            schema: None,
            table_state,
            selected_col: 0,
            col_offset: 0,
            collapsed_cols: HashSet::new(),
            col_widths: HashMap::new(),
            read_only: false,
            status: Some("Loading…".into()),
            export_prompt: false,
            filters: BTreeMap::new(),
            filter_input: None,
            loaded_count: 0,
            total_count: None,
            has_more: true,
            loading: true,
        }
    }

    // Initial/replacement load (resets everything)
    pub fn set_result(&mut self, result: DbQueryResult) {
        let count = result.rows.len();
        self.has_more = count == PAGE_SIZE;
        self.loaded_count = count;
        self.status = None;
        self.loading = false;
        self.selected_col = 0;
        self.col_offset = 0;
        self.collapsed_cols.clear();
        self.col_widths.clear();
        self.table_state = TableState::default();
        self.table_state.select(if count > 0 { Some(0) } else { None });
        self.result = Some(result);
    }

    // Append next page rows
    pub fn append_rows(&mut self, result: DbQueryResult) {
        let new_count = result.rows.len();
        self.has_more = new_count == PAGE_SIZE;
        self.loading = false;
        self.loaded_count += new_count;
        if let Some(ref mut existing) = self.result {
            existing.rows.extend(result.rows);
        } else {
            self.table_state.select(Some(0));
            self.result = Some(result);
        }
    }

    // Update COUNT(*) result
    pub fn set_total(&mut self, count: u64) {
        self.total_count = Some(count);
    }

    // Reset data but keep table_name, filters, selected_col, collapsed_cols
    pub fn reset_data(&mut self) {
        self.result = None;
        self.table_state = TableState::default();
        self.col_offset = 0;
        self.status = Some("Loading…".into());
        self.loaded_count = 0;
        self.total_count = None;
        self.has_more = true;
        self.loading = true;
        self.filter_input = None;
    }

    pub fn set_error(&mut self, msg: String) {
        self.status = Some(format!("Error: {msg}"));
        self.loading = false;
    }

    // ── Key handling ──────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: KeyEvent) -> DataGridAction {
        // Export prompt mode
        if self.export_prompt {
            self.export_prompt = false;
            return match key.code {
                KeyCode::Char('c') | KeyCode::Char('C') => DataGridAction::ExportCsv,
                KeyCode::Char('j') | KeyCode::Char('J') => DataGridAction::ExportJson,
                _ => DataGridAction::None,
            };
        }

        // Filter input mode takes priority
        if let Some(ref mut fi) = self.filter_input {
            return match key.code {
                KeyCode::Esc => {
                    self.filter_input = None;
                    DataGridAction::None
                }
                KeyCode::Enter => {
                    let col_name = fi.col_name.clone();
                    let value = fi.value.trim().to_string();
                    self.filter_input = None;
                    if value.is_empty() {
                        self.filters.remove(&col_name);
                    } else {
                        self.filters.insert(col_name, value);
                    }
                    DataGridAction::ApplyFilter
                }
                KeyCode::Backspace => {
                    fi.value.pop();
                    DataGridAction::None
                }
                KeyCode::Char(c) => {
                    fi.value.push(c);
                    DataGridAction::None
                }
                _ => DataGridAction::None,
            };
        }

        // Normal mode
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => DataGridAction::Back,

            KeyCode::Char('j') | KeyCode::Down => {
                let was_last = self.is_at_last_row();
                self.move_row(1);
                if was_last && self.has_more && !self.loading {
                    DataGridAction::LoadMore
                } else {
                    DataGridAction::None
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_row(-1);
                DataGridAction::None
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.move_col(1);
                DataGridAction::None
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.move_col(-1);
                DataGridAction::None
            }
            KeyCode::Char('g') => {
                self.go_first();
                DataGridAction::None
            }
            KeyCode::Char('G') => {
                self.go_last();
                DataGridAction::None
            }
            KeyCode::PageDown => {
                self.move_row(10);
                DataGridAction::None
            }
            KeyCode::PageUp => {
                self.move_row(-10);
                DataGridAction::None
            }
            KeyCode::Char(' ') => {
                self.toggle_collapse();
                DataGridAction::None
            }

            // Filter: open input for selected column
            KeyCode::Char('f') if !self.read_only => {
                self.start_filter();
                DataGridAction::None
            }
            // Remove filter on selected column
            KeyCode::Char('d') if !self.read_only => {
                if let Some(name) = self.selected_col_name() {
                    if self.filters.remove(&name).is_some() {
                        return DataGridAction::ApplyFilter;
                    }
                }
                DataGridAction::None
            }
            // Clear all filters
            KeyCode::Char('F') if !self.read_only => {
                if !self.filters.is_empty() {
                    self.filters.clear();
                    DataGridAction::ApplyFilter
                } else {
                    DataGridAction::None
                }
            }

            KeyCode::Enter if !self.read_only => DataGridAction::EnterCell,

            // Manual column resize
            // Export prompt
            KeyCode::Char('E') => {
                if self.result.is_some() {
                    self.export_prompt = true;
                }
                DataGridAction::None
            }

            KeyCode::Char('[') => {
                let current = self.effective_col_width(self.selected_col);
                self.col_widths.insert(
                    self.selected_col,
                    current.saturating_sub(COL_RESIZE_STEP).max(4),
                );
                DataGridAction::None
            }
            KeyCode::Char(']') => {
                let current = self.effective_col_width(self.selected_col);
                self.col_widths.insert(
                    self.selected_col,
                    (current + COL_RESIZE_STEP).min(80),
                );
                DataGridAction::None
            }

            _ => DataGridAction::None,
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn start_filter(&mut self) {
        if let Some(name) = self.selected_col_name() {
            let existing = self.filters.get(&name).cloned().unwrap_or_default();
            self.filter_input = Some(FilterInput { col_name: name, value: existing });
        }
    }

    fn selected_col_name(&self) -> Option<String> {
        self.result.as_ref()?.columns.get(self.selected_col).map(|c| c.name.clone())
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

    fn is_at_last_row(&self) -> bool {
        let count = self.row_count();
        count > 0 && self.selected_row() >= count - 1
    }

    fn effective_col_width(&self, col_idx: usize) -> u16 {
        if self.collapsed_cols.contains(&col_idx) {
            return COLLAPSED_WIDTH;
        }
        if let Some(&w) = self.col_widths.get(&col_idx) {
            return w;
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
        let mut used = 1u16;
        for i in self.col_offset..col_count {
            let w = self.effective_col_width(i);
            used += w + 1;
            if used > available_width {
                break;
            }
            visible.push(i);
        }
        visible
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
        if next < self.col_offset {
            self.col_offset = next;
        }
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

    // ── Draw ──────────────────────────────────────────────────────────────────

    pub fn draw(f: &mut Frame<'_>, screen: &mut DataGridScreen, area: Rect) {

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // info bar
                Constraint::Min(0),    // data table
                Constraint::Length(2), // cell preview
                Constraint::Length(3), // help / filter bar
            ])
            .split(area);

        // ── Info bar ──────────────────────────────────────────────────────────
        let row_info = screen.result.as_ref().map_or_else(
            || screen.status.clone().unwrap_or_default(),
            |r| {
                if r.rows.is_empty() {
                    "no rows".to_string()
                } else {
                    format!("row {}/{}", screen.selected_row() + 1, r.rows.len())
                }
            },
        );
        let col_info = screen.result.as_ref().map_or(String::new(), |r| {
            format!("col {}/{}", screen.selected_col + 1, r.columns.len())
        });
        let count_info = match (screen.total_count, screen.has_more) {
            (Some(total), _) => format!("{}/{} rows", screen.loaded_count, total),
            (None, true)     => format!("{}+ rows", screen.loaded_count),
            (None, false)    => format!("{} rows", screen.loaded_count),
        };
        let filter_info = if screen.filters.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = screen.filters.iter()
                .map(|(k, v)| format!("[{}≈{}]", k, v))
                .collect();
            format!("  {}", parts.join(" "))
        };
        let loading_label = if screen.loading { "  ⏳" } else { "" };

        let shown_name = screen.display_name.as_deref().unwrap_or(&screen.table_name);
        f.render_widget(
            Paragraph::new(format!(
                " {} │ {} │ {} │ {}{}{}",
                shown_name, row_info, col_info, count_info, filter_info, loading_label
            ))
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            chunks[0],
        );

        // ── Adjust col_offset ─────────────────────────────────────────────────
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

        // ── Data table ────────────────────────────────────────────────────────
        match &screen.result {
            None => {
                let msg = screen.status.clone().unwrap_or_else(|| "Loading…".into());
                f.render_widget(
                    Paragraph::new(msg)
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
            }
            Some(result) if result.rows.is_empty() => {
                f.render_widget(
                    Paragraph::new("Table is empty (or no rows match the current filters)")
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
            }
            Some(result) => {
                let widths: Vec<Constraint> = visible
                    .iter()
                    .map(|&i| Constraint::Length(screen.effective_col_width(i)))
                    .collect();

                let header_cells: Vec<Cell> = visible
                    .iter()
                    .map(|&i| {
                        let col_name = &result.columns[i].name;
                        let is_filtered = screen.filters.contains_key(col_name);
                        let name = if screen.collapsed_cols.contains(&i) {
                            "…".to_string()
                        } else {
                            truncate(col_name, screen.effective_col_width(i) as usize)
                        };
                        // Selected + filtered → yellow underlined
                        // Selected only       → yellow underlined
                        // Filtered only       → cyan bold
                        // Normal              → bold
                        let style = if i == screen.selected_col {
                            let base = Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                            if is_filtered { base.bg(Color::DarkGray) } else { base }
                        } else if is_filtered {
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().add_modifier(Modifier::BOLD)
                        };
                        Cell::from(name).style(style)
                    })
                    .collect();

                let header = RatRow::new(header_cells)
                    .style(Style::default().bg(Color::DarkGray))
                    .height(1);

                let sel_row = screen.table_state.selected().unwrap_or(usize::MAX);
                let sel_col = screen.selected_col;

                // Build column-name → FK table map from schema (if loaded)
                let fk_map: HashMap<&str, &str> = screen.schema.as_ref()
                    .map(|s| s.iter()
                        .filter_map(|cs| cs.fk.as_ref().map(|fk| (cs.name.as_str(), fk.table.as_str())))
                        .collect())
                    .unwrap_or_default();

                let data_rows: Vec<RatRow> = result
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(row_idx, row)| {
                        let is_sel_row = row_idx == sel_row;
                        let cells: Vec<Cell> = visible
                            .iter()
                            .map(|&col_idx| {
                                let val = row.values.get(col_idx).unwrap_or(&Value::Null);
                                let s = value_display(val);
                                let col_width = screen.effective_col_width(col_idx) as usize;
                                let col_name = result.columns[col_idx].name.as_str();
                                let fk_table = fk_map.get(col_name).copied();

                                let (val_display, badge) = if screen.collapsed_cols.contains(&col_idx) {
                                    let abbrev = s.chars().next()
                                        .map(|c| format!("{c}…"))
                                        .unwrap_or_else(|| "…".into());
                                    (abbrev, String::new())
                                } else if let Some(tbl) = fk_table {
                                    // Always show the FK badge; truncate the value to whatever
                                    // space remains (can be 0 if the badge fills the column).
                                    let badge = format!(" [{}]", tbl);
                                    let avail = col_width.saturating_sub(badge.len());
                                    (truncate(&s, avail), badge)
                                } else {
                                    (truncate(&s, col_width), String::new())
                                };

                                let style = if is_sel_row && col_idx == sel_col {
                                    Style::default()
                                        .fg(Color::White)
                                        .bg(Color::Blue)
                                        .add_modifier(Modifier::BOLD)
                                } else if is_sel_row {
                                    Style::default()
                                        .fg(Color::Black)
                                        .bg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD)
                                } else if matches!(val, Value::Null) {
                                    Style::default().fg(Color::DarkGray)
                                } else {
                                    Style::default()
                                };

                                let badge_style = Style::default().fg(Color::Magenta);
                                let cell_line = Line::from(vec![
                                    Span::styled(val_display, style),
                                    Span::styled(badge, badge_style),
                                ]);
                                Cell::from(cell_line).style(style)
                            })
                            .collect();
                        RatRow::new(cells).height(1)
                    })
                    .collect();

                let table = Table::new(data_rows, widths)
                    .header(header)
                    .block(Block::default().borders(Borders::ALL))
                    // highlight_style is transparent: per-cell styles above handle it
                    .highlight_style(Style::default())
                    .highlight_symbol("> ");

                f.render_stateful_widget(table, chunks[1], &mut screen.table_state);
            }
        }

        // ── Cell preview ──────────────────────────────────────────────────────
        let preview_text = screen.result.as_ref().and_then(|r| {
            let row_idx = screen.table_state.selected()?;
            let row = r.rows.get(row_idx)?;
            let col_name = r.columns.get(screen.selected_col).map(|c| c.name.as_str()).unwrap_or("");
            let val = row.values.get(screen.selected_col).unwrap_or(&Value::Null);
            Some(format!(" ▸ {} : {}", col_name, value_display(val)))
        }).unwrap_or_default();
        f.render_widget(
            Paragraph::new(preview_text)
                .style(Style::default().fg(Color::White).bg(Color::DarkGray)),
            chunks[2],
        );

        // ── Help / filter bar ─────────────────────────────────────────────────
        if screen.export_prompt {
            f.render_widget(
                Paragraph::new(" Export:  c = CSV   j = JSON   Esc = cancel ")
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::Yellow)),
                chunks[3],
            );
        } else if let Some(ref fi) = screen.filter_input {
            let prompt = format!(" Filter [{}] > {}", fi.col_name, fi.value);
            f.render_widget(
                Paragraph::new(prompt.clone())
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::Yellow)),
                chunks[3],
            );
            f.set_cursor(
                chunks[3].x + 1 + prompt.len() as u16,
                chunks[3].y + 1,
            );
        } else if screen.read_only {
            f.render_widget(
                Paragraph::new(
                    " j/k: rows   h/l: cols   [/]: resize   g/G: first/last   Space: collapse   E: export   q: back"
                )
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray)),
                chunks[3],
            );
        } else {
            let collapse_label = if screen.collapsed_cols.contains(&screen.selected_col) {
                "Space: expand"
            } else {
                "Space: collapse"
            };
            let filter_hint = if screen.filters.is_empty() {
                "f: filter col"
            } else {
                "f: edit filter   d: rm col filter   F: clear all"
            };
            f.render_widget(
                Paragraph::new(format!(
                    " j/k: rows   h/l: cols   [/]: resize   g/G: first/last   Enter: cell   {}   {}   E: export   q: back",
                    collapse_label, filter_hint
                ))
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray)),
                chunks[3],
            );
        }
    }
}

// ── Formatting helpers ────────────────────────────────────────────────────────

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
    let max = max_chars.saturating_sub(1);
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
