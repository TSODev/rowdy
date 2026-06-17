use crossterm::event::KeyEvent;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row as RatRow, Table, TableState},
    Frame,
};
use tui_textarea::{Input, Key, TextArea};
use crate::db::types::{DbQueryResult, Value};

// ── SQL keywords for autocomplete ────────────────────────────────────────────

const SQL_KEYWORDS: &[&str] = &[
    // DML
    "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "IN", "EXISTS", "BETWEEN", "LIKE", "ILIKE",
    "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "RETURNING", "MERGE", "UPSERT",
    // Clauses
    "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "FULL", "CROSS", "NATURAL", "ON", "USING",
    "GROUP", "ORDER", "BY", "HAVING", "LIMIT", "OFFSET", "FETCH", "NEXT", "ROWS", "ONLY",
    "UNION", "INTERSECT", "EXCEPT", "ALL", "DISTINCT",
    "AS", "WITH", "RECURSIVE",
    // DDL
    "CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME", "TABLE", "INDEX", "VIEW", "SCHEMA",
    "DATABASE", "SEQUENCE", "TRIGGER", "PROCEDURE", "FUNCTION",
    "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "UNIQUE", "DEFAULT", "CHECK", "CONSTRAINT",
    "NOT NULL", "NULL", "ASC", "DESC", "NULLS", "FIRST", "LAST",
    // Conditions / flow
    "CASE", "WHEN", "THEN", "ELSE", "END", "IF",
    "IS", "NULL", "TRUE", "FALSE",
    // Aggregates & functions
    "COUNT", "SUM", "AVG", "MIN", "MAX",
    "COALESCE", "NULLIF", "IFNULL", "NVL",
    "CAST", "CONVERT", "EXTRACT",
    "NOW", "CURRENT_TIMESTAMP", "CURRENT_DATE", "CURRENT_TIME",
    "CONCAT", "LENGTH", "LOWER", "UPPER", "TRIM", "SUBSTRING", "REPLACE",
    "ABS", "ROUND", "FLOOR", "CEIL",
    // Window functions
    "OVER", "PARTITION", "ROW_NUMBER", "RANK", "DENSE_RANK", "LAG", "LEAD",
    "FIRST_VALUE", "LAST_VALUE", "NTILE",
    // Transactions
    "BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT", "TRANSACTION",
    // Meta
    "EXPLAIN", "ANALYZE", "SHOW", "DESCRIBE", "PRAGMA",
    // Types
    "INT", "INTEGER", "BIGINT", "SMALLINT", "FLOAT", "DOUBLE", "REAL",
    "DECIMAL", "NUMERIC", "CHAR", "VARCHAR", "TEXT", "BLOB",
    "BOOLEAN", "BOOL", "DATE", "TIME", "TIMESTAMP", "INTERVAL",
    "UUID", "JSON", "JSONB", "ARRAY", "SERIAL", "BYTEA",
];

// ── Focus ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum EditorFocus {
    Editor,
    Results,
}

// ── Query result ──────────────────────────────────────────────────────────────

pub enum QueryResult {
    Rows(DbQueryResult),
    Affected(u64),
    Error(String),
}

// ── Actions ───────────────────────────────────────────────────────────────────

pub enum SqlEditorAction {
    None,
    Execute(String),
    Back,
    OpenGrid(DbQueryResult),
    HistoryPrev,
    HistoryNext,
}

// ── Autocomplete ──────────────────────────────────────────────────────────────

struct AutocompletePopup {
    suggestions: Vec<String>,
    selected: usize,
    prefix: String,
}

// ── Screen ────────────────────────────────────────────────────────────────────

pub struct SqlEditorScreen {
    pub editor: TextArea<'static>,
    pub result: Option<QueryResult>,
    pub result_state: TableState,
    pub result_col_offset: usize,
    pub focus: EditorFocus,
    pub running: bool,
    pub db_info: String,
    pub nosql_collection: Option<String>,
    // Autocomplete
    completion_items: Vec<String>,
    autocomplete: Option<AutocompletePopup>,
}

impl SqlEditorScreen {
    pub fn new(db_info: String) -> Self {
        let mut editor = TextArea::default();
        editor.set_cursor_line_style(Style::default());
        editor.set_placeholder_text("-- Write your SQL here…");
        editor.set_placeholder_style(Style::default().fg(Color::Gray));
        Self {
            editor,
            result: None,
            result_state: TableState::default(),
            result_col_offset: 0,
            focus: EditorFocus::Editor,
            running: false,
            db_info,
            nosql_collection: None,
            completion_items: Vec::new(),
            autocomplete: None,
        }
    }

    pub fn set_completions(&mut self, items: Vec<String>) {
        self.completion_items = items;
    }

    pub fn set_nosql_collection(&mut self, collection: Option<String>) {
        self.nosql_collection = collection;
        let placeholder = if self.nosql_collection.is_some() {
            "-- MQL filter: {\"field\": value}  or pipeline: [{\"$match\": {…}}]"
        } else {
            "-- Write your SQL here…"
        };
        self.editor.set_placeholder_text(placeholder);
    }

    pub fn set_rows(&mut self, result: DbQueryResult) {
        self.running = false;
        self.result_state = TableState::default();
        if !result.rows.is_empty() {
            self.result_state.select(Some(0));
        }
        self.result_col_offset = 0;
        self.result = Some(QueryResult::Rows(result));
    }

    pub fn set_affected(&mut self, n: u64) {
        self.running = false;
        self.result = Some(QueryResult::Affected(n));
        self.result_state = TableState::default();
    }

    pub fn set_error(&mut self, msg: String) {
        self.running = false;
        self.result = Some(QueryResult::Error(msg));
        self.result_state = TableState::default();
    }

    pub fn set_running(&mut self) {
        self.running = true;
        self.result = None;
    }

    pub fn set_editor_content(&mut self, text: &str) {
        let lines: Vec<String> = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(|l| l.to_string()).collect()
        };
        let mut ta = TextArea::from(lines);
        ta.set_cursor_line_style(Style::default());
        ta.set_placeholder_text("-- Write your SQL here…");
        ta.set_placeholder_style(Style::default().fg(Color::Gray));
        self.editor = ta;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SqlEditorAction {
        let input = Input::from(key);
        match self.focus {
            EditorFocus::Editor => {
                // ── Autocomplete popup active ─────────────────────────────────
                if self.autocomplete.is_some() {
                    match input {
                        Input { key: Key::Esc, .. } => {
                            self.autocomplete = None;
                        }
                        Input { key: Key::Up, .. } => {
                            if let Some(ref mut ac) = self.autocomplete {
                                if ac.selected > 0 { ac.selected -= 1; }
                            }
                        }
                        Input { key: Key::Down, .. } | Input { key: Key::Tab, .. } => {
                            if let Some(ref mut ac) = self.autocomplete {
                                if ac.selected + 1 < ac.suggestions.len() {
                                    ac.selected += 1;
                                }
                            }
                        }
                        Input { key: Key::Enter, .. } => {
                            if let Some(ac) = self.autocomplete.take() {
                                let completion = ac.suggestions[ac.selected].clone();
                                // Delete the typed prefix
                                for _ in 0..ac.prefix.chars().count() {
                                    self.editor.input(Input { key: Key::Backspace, ctrl: false, alt: false, shift: false });
                                }
                                // Insert the completion
                                for c in completion.chars() {
                                    self.editor.input(Input { key: Key::Char(c), ctrl: false, alt: false, shift: false });
                                }
                            }
                        }
                        // Any other key closes popup and passes through normally
                        _ => {
                            self.autocomplete = None;
                            self.editor.input(input);
                            self.refresh_autocomplete();
                        }
                    }
                    return SqlEditorAction::None;
                }

                // ── Normal editor mode ────────────────────────────────────────
                match input {
                    // Execute: F5 or Ctrl+Enter
                    Input { key: Key::F(5), .. }
                    | Input { key: Key::Enter, ctrl: true, .. } => {
                        let sql = self.editor.lines().join("\n");
                        let sql = sql.trim().to_string();
                        if !sql.is_empty() && !self.running {
                            return SqlEditorAction::Execute(sql);
                        }
                    }
                    // Back: Ctrl+Q
                    Input { key: Key::Char('q'), ctrl: true, .. } => {
                        return SqlEditorAction::Back;
                    }
                    // Open result in full grid: F4
                    Input { key: Key::F(4), .. } => {
                        if let Some(QueryResult::Rows(r)) = &self.result {
                            return SqlEditorAction::OpenGrid(r.clone());
                        }
                    }
                    // Tab: autocomplete or switch focus to results
                    Input { key: Key::Tab, .. } => {
                        let prefix = self.word_at_cursor();
                        let suggestions = self.filter_completions(&prefix);
                        if !suggestions.is_empty() {
                            self.autocomplete = Some(AutocompletePopup {
                                suggestions,
                                selected: 0,
                                prefix,
                            });
                        } else if self.result.is_some() {
                            self.focus = EditorFocus::Results;
                        }
                    }
                    // History navigation: Alt+Up (older) / Alt+Down (newer)
                    Input { key: Key::Up, alt: true, .. } => {
                        return SqlEditorAction::HistoryPrev;
                    }
                    Input { key: Key::Down, alt: true, .. } => {
                        return SqlEditorAction::HistoryNext;
                    }
                    // Regular input — pass to textarea, then refresh autocomplete
                    _ => {
                        self.editor.input(input);
                        self.refresh_autocomplete();
                    }
                }
            }
            EditorFocus::Results => {
                match input {
                    Input { key: Key::Tab, .. }
                    | Input { key: Key::Esc, .. } => {
                        self.focus = EditorFocus::Editor;
                    }
                    Input { key: Key::F(4), .. } => {
                        if let Some(QueryResult::Rows(r)) = &self.result {
                            return SqlEditorAction::OpenGrid(r.clone());
                        }
                    }
                    Input { key: Key::Char('j'), .. }
                    | Input { key: Key::Down, .. }   => self.result_move_row(1),
                    Input { key: Key::Char('k'), .. }
                    | Input { key: Key::Up, .. }     => self.result_move_row(-1),
                    Input { key: Key::Char('l'), .. }
                    | Input { key: Key::Right, .. }  => self.result_move_col(1),
                    Input { key: Key::Char('h'), .. }
                    | Input { key: Key::Left, .. }   => self.result_move_col(-1),
                    Input { key: Key::Char('g'), .. } => self.result_go_first(),
                    Input { key: Key::Char('G'), .. } => self.result_go_last(),
                    Input { key: Key::PageDown, .. } => self.result_move_row(10),
                    Input { key: Key::PageUp, .. }   => self.result_move_row(-10),
                    _ => {}
                }
            }
        }
        SqlEditorAction::None
    }

    // ── Autocomplete helpers ──────────────────────────────────────────────────

    fn word_at_cursor(&self) -> String {
        let lines = self.editor.lines();
        let (row, col) = self.editor.cursor();
        if row >= lines.len() { return String::new(); }
        let chars: Vec<char> = lines[row].chars().collect();
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        chars[start..col].iter().collect()
    }

    fn filter_completions(&self, prefix: &str) -> Vec<String> {
        if prefix.len() < 2 { return vec![]; }
        let lower = prefix.to_lowercase();

        // Schema items (tables + columns) have priority
        let schema: Vec<String> = self.completion_items.iter()
            .filter(|s| s.to_lowercase().starts_with(&lower))
            .take(7)
            .cloned()
            .collect();

        // Fill remaining slots with SQL keywords (case-insensitive match, returned uppercase)
        let remaining = 10usize.saturating_sub(schema.len());
        let keywords: Vec<String> = SQL_KEYWORDS.iter()
            .filter(|k| k.to_lowercase().starts_with(&lower) && !schema.contains(&k.to_string()))
            .take(remaining)
            .map(|k| k.to_string())
            .collect();

        let mut result = schema;
        result.extend(keywords);
        result
    }

    fn refresh_autocomplete(&mut self) {
        if self.autocomplete.is_none() { return; }
        let prefix = self.word_at_cursor();
        let suggestions = self.filter_completions(&prefix);
        if suggestions.is_empty() {
            self.autocomplete = None;
        } else if let Some(ref mut ac) = self.autocomplete {
            ac.selected = ac.selected.min(suggestions.len().saturating_sub(1));
            ac.prefix = prefix;
            ac.suggestions = suggestions;
        }
    }

    fn result_row_count(&self) -> usize {
        match &self.result {
            Some(QueryResult::Rows(r)) => r.rows.len(),
            _ => 0,
        }
    }

    fn result_col_count(&self) -> usize {
        match &self.result {
            Some(QueryResult::Rows(r)) => r.columns.len(),
            _ => 0,
        }
    }

    fn result_selected_row(&self) -> usize {
        self.result_state.selected().unwrap_or(0)
    }

    fn result_move_row(&mut self, delta: i64) {
        let count = self.result_row_count();
        if count == 0 { return; }
        let next = (self.result_selected_row() as i64 + delta)
            .clamp(0, count as i64 - 1) as usize;
        self.result_state.select(Some(next));
    }

    fn result_move_col(&mut self, delta: i64) {
        let count = self.result_col_count();
        if count == 0 { return; }
        let next = (self.result_col_offset as i64 + delta)
            .clamp(0, count as i64 - 1) as usize;
        self.result_col_offset = next;
    }

    fn result_go_first(&mut self) {
        if self.result_row_count() > 0 {
            self.result_state.select(Some(0));
        }
    }

    fn result_go_last(&mut self) {
        let n = self.result_row_count();
        if n > 0 {
            self.result_state.select(Some(n - 1));
        }
    }

    pub fn draw(f: &mut Frame<'_>, screen: &mut SqlEditorScreen, area: Rect) {

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Min(3),
                Constraint::Length(3),
            ])
            .split(area);

        draw_editor(f, screen, vertical[0]);
        draw_results(f, screen, vertical[1]);
        draw_help(f, screen, vertical[2]);
    }
}

// ── Editor pane ───────────────────────────────────────────────────────────────

fn draw_editor(f: &mut Frame<'_>, screen: &mut SqlEditorScreen, area: Rect) {
    let focused = screen.focus == EditorFocus::Editor;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let status_label = if screen.running { " ⏳ " } else { "" };
    let title = if let Some(ref coll) = screen.nosql_collection {
        format!(" MQL Editor {}│ {} │ collection: {} ", status_label, screen.db_info, coll)
    } else {
        format!(" SQL Editor {}│ {} ", status_label, screen.db_info)
    };
    screen.editor.set_block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style),
    );
    screen.editor.set_selection_style(
        Style::default().bg(Color::DarkGray).fg(Color::White),
    );
    f.render_widget(&screen.editor, area);

    // ── Autocomplete popup ────────────────────────────────────────────────────
    if let Some(ref ac) = screen.autocomplete {
        let (cursor_row, cursor_col) = screen.editor.cursor();
        // Position just below the cursor (inside the editor border)
        let origin_x = area.x + 1 + cursor_col.saturating_sub(ac.prefix.len()) as u16;
        let origin_y = area.y + 1 + cursor_row as u16 + 1; // +1 = line below cursor

        let max_visible = 8usize;
        let visible_count = ac.suggestions.len().min(max_visible);
        let popup_w = (ac.suggestions.iter().map(|s| s.len()).max().unwrap_or(8) as u16 + 4)
            .min(area.width.saturating_sub(2));
        let popup_h = visible_count as u16 + 2; // border top + bottom

        // Clamp to stay inside the terminal area
        let popup_x = origin_x.min(area.x + area.width.saturating_sub(popup_w));
        let popup_y = if origin_y + popup_h > area.y + area.height {
            // Not enough space below — show above cursor instead
            (area.y + 1 + cursor_row as u16).saturating_sub(popup_h)
        } else {
            origin_y
        };

        let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);
        f.render_widget(Clear, popup_area);

        let scroll_offset = ac.selected.saturating_sub(max_visible - 1);
        let rows: Vec<RatRow> = ac.suggestions.iter().enumerate()
            .skip(scroll_offset)
            .take(max_visible)
            .map(|(i, s)| {
                let style = if i == ac.selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                RatRow::new(vec![Cell::from(format!(" {s} ")).style(style)])
            })
            .collect();

        let hint = format!(" {}/{} ", ac.selected + 1, ac.suggestions.len());
        f.render_widget(
            Table::new(rows, vec![Constraint::Fill(1)])
                .block(
                    Block::default()
                        .title(hint)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
            popup_area,
        );
    }
}

// ── Results pane ──────────────────────────────────────────────────────────────

fn draw_results(f: &mut Frame<'_>, screen: &mut SqlEditorScreen, area: Rect) {
    let focused = screen.focus == EditorFocus::Results;
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    match &screen.result {
        None => {
            let msg = if screen.running {
                "Executing query…"
            } else {
                "Press F5 or Ctrl+Enter to run a query"
            };
            f.render_widget(
                Paragraph::new(msg)
                    .block(
                        Block::default()
                            .title(" Results ")
                            .borders(Borders::ALL)
                            .border_style(border_style),
                    )
                    .style(Style::default().fg(Color::Gray)),
                area,
            );
        }

        Some(QueryResult::Affected(n)) => {
            f.render_widget(
                Paragraph::new(format!("  {} row(s) affected", n))
                    .block(
                        Block::default()
                            .title(" Results ")
                            .borders(Borders::ALL)
                            .border_style(border_style),
                    )
                    .style(Style::default().fg(Color::Green)),
                area,
            );
        }

        Some(QueryResult::Error(msg)) => {
            let msg = msg.clone();
            let inner_width = area.width.saturating_sub(4) as usize;
            let wrapped = word_wrap(&format!("Error: {}", msg), inner_width.max(20));
            f.render_widget(
                Paragraph::new(wrapped)
                    .block(
                        Block::default()
                            .title(" Results ")
                            .borders(Borders::ALL)
                            .border_style(border_style),
                    )
                    .style(Style::default().fg(Color::Red)),
                area,
            );
        }

        Some(QueryResult::Rows(result)) => {
            let col_count = result.columns.len();
            let col_offset = screen.result_col_offset.min(col_count.saturating_sub(1));
            let available_w = area.width.saturating_sub(4);

            let mut visible_cols: Vec<usize> = vec![];
            let mut used = 0u16;
            for i in col_offset..col_count {
                let w = col_display_width(result, i);
                if used + w + 1 > available_w {
                    break;
                }
                used += w + 1;
                visible_cols.push(i);
            }

            let widths: Vec<Constraint> = visible_cols
                .iter()
                .map(|&i| Constraint::Length(col_display_width(result, i)))
                .collect();

            let header_cells: Vec<Cell> = visible_cols
                .iter()
                .map(|&i| {
                    Cell::from(truncate_str(
                        &result.columns[i].name,
                        col_display_width(result, i) as usize,
                    ))
                    .style(Style::default().add_modifier(Modifier::BOLD))
                })
                .collect();

            let header = RatRow::new(header_cells)
                .style(Style::default().bg(Color::DarkGray))
                .height(1);

            // Clone rows/cols to avoid borrow issues with result_state below
            let rows_data: Vec<Vec<String>> = result
                .rows
                .iter()
                .map(|row| {
                    visible_cols
                        .iter()
                        .map(|&i| {
                            let val = row.values.get(i).unwrap_or(&Value::Null);
                            truncate_str(
                                &value_str(val),
                                col_display_width(result, i) as usize,
                            )
                        })
                        .collect()
                })
                .collect();

            let row_count = result.rows.len();
            let title = format!(
                " Results: {} row{} ",
                row_count,
                if row_count == 1 { "" } else { "s" }
            );

            let data_rows: Vec<RatRow> = rows_data
                .into_iter()
                .map(|cells| RatRow::new(cells).height(1))
                .collect();

            let table = Table::new(data_rows, widths)
                .header(header)
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(border_style),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(table, area, &mut screen.result_state);
        }
    }
}

// ── Help bar ──────────────────────────────────────────────────────────────────

fn draw_help(f: &mut Frame<'_>, screen: &SqlEditorScreen, area: Rect) {
    let text = match screen.focus {
        EditorFocus::Editor =>
            " F5/Ctrl+Enter: run   Alt+↑/↓: history   Tab: results   F4: grid   Ctrl+Q: back ",
        EditorFocus::Results =>
            " j/k: rows   h/l: cols   g/G: first/last   PgUp/Dn: page   F4: open in grid   Tab/Esc: editor ",
    };
    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray)),
        area,
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn word_wrap(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut line_len = 0usize;
    for word in text.split_whitespace() {
        if line_len == 0 {
            out.push_str(word);
            line_len = word.len();
        } else if line_len + 1 + word.len() <= width {
            out.push(' ');
            out.push_str(word);
            line_len += 1 + word.len();
        } else {
            out.push('\n');
            out.push_str(word);
            line_len = word.len();
        }
    }
    out
}

fn col_display_width(result: &DbQueryResult, col_idx: usize) -> u16 {
    let header_w = result.columns[col_idx].name.len() as u16;
    let max_val_w = result
        .rows
        .iter()
        .map(|r| value_str(r.values.get(col_idx).unwrap_or(&Value::Null)).len() as u16)
        .max()
        .unwrap_or(0);
    (header_w.max(max_val_w) + 2).min(30)
}

fn value_str(v: &Value) -> String {
    match v {
        Value::Null                              => "NULL".into(),
        Value::Bool(b)                           => b.to_string(),
        Value::Int(i)                            => i.to_string(),
        Value::Float(f)                          => format!("{f:.4}"),
        Value::Text(s)                           => s.replace('\n', "↵").replace('\r', ""),
        Value::Bytes(b)                          => format!("<{} bytes>", b.len()),
        Value::NestedDoc(s) | Value::NestedArray(s) => s.clone(),
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    let cut = max.saturating_sub(1);
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(cut).collect();
        format!("{t}…")
    }
}
