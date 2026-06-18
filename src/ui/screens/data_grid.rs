use std::collections::{BTreeMap, HashMap, HashSet};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

// ── Search state ──────────────────────────────────────────────────────────────

pub struct SearchState {
    pub query: String,
    pub matches: Vec<(usize, usize)>, // (row_idx, col_idx)
    pub current: usize,
    pub prompt_open: bool,
}

// ── Actions ───────────────────────────────────────────────────────────────────

pub enum DataGridAction {
    None,
    Back,
    ApplyFilter,
    ApplySort,
    LoadMore,
    LoadAll,
    EnterCell,
    ExportCsv,
    ExportJson,
    ExportJsonSimple,
    InsertMongo,
    DeleteMongo,
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
    pub prod_readonly: bool,
    pub is_view: bool,
    pub is_nosql: bool,
    pub status: Option<String>,
    pub export_prompt: bool,
    // Filtering
    pub filters: BTreeMap<String, String>,
    pub filter_input: Option<FilterInput>,
    // Sorting
    pub sort_col_name: Option<String>,
    pub sort_asc: bool,
    pub sortable: bool,
    // Pagination
    pub loaded_count: usize,
    pub total_count: Option<u64>,
    pub has_more: bool,
    pub loading: bool,
    // Preserved cursor across reloads (filter/sort/edit-save)
    preserved_row: Option<usize>,
    // Full-text search
    pub search: Option<SearchState>,
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
            prod_readonly: false,
            is_view: false,
            is_nosql: false,
            status: Some("Loading…".into()),
            export_prompt: false,
            filters: BTreeMap::new(),
            filter_input: None,
            sort_col_name: None,
            sort_asc: true,
            sortable: false,
            loaded_count: 0,
            total_count: None,
            has_more: true,
            loading: true,
            preserved_row: None,
            search: None,
        }
    }

    // Initial/replacement load — preserves col position always; restores row when
    // set by reset_data() (filter/sort/edit-save reloads), defaults to 0 for new tables.
    pub fn set_result(&mut self, result: DbQueryResult) {
        let count = result.rows.len();
        self.has_more = count == PAGE_SIZE;
        self.loaded_count = count;
        self.status = None;
        self.loading = false;
        // selected_col and col_offset are intentionally NOT reset here
        self.collapsed_cols.clear();
        self.col_widths.clear();
        let row = self.preserved_row.take()
            .map(|r| if count > 0 { r.min(count - 1) } else { 0 })
            .unwrap_or(0);
        self.table_state = TableState::default();
        self.table_state.select(if count > 0 { Some(row) } else { None });
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

    // Reset data but keep table_name, filters, selected_col, col_offset, collapsed_cols.
    // Saves the current row so set_result() can restore it after the reload.
    pub fn reset_data(&mut self) {
        self.preserved_row = self.table_state.selected();
        self.result = None;
        self.table_state = TableState::default();
        self.status = Some("Loading…".into());
        self.loaded_count = 0;
        self.total_count = None;
        self.has_more = true;
        self.loading = true;
        self.filter_input = None;
        self.search = None;
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
                KeyCode::Char('j') => DataGridAction::ExportJsonSimple,
                KeyCode::Char('J') => DataGridAction::ExportJson,
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

        // Search prompt mode
        if self.search.as_ref().is_some_and(|s| s.prompt_open) {
            return self.handle_search_key(key);
        }

        // Normal mode
        match key.code {
            KeyCode::Char('q') => DataGridAction::Back,
            KeyCode::Esc => {
                if self.search.is_some() {
                    self.search = None;
                    DataGridAction::None
                } else {
                    DataGridAction::Back
                }
            }

            // Search navigation (prompt closed, matches available)
            KeyCode::Char('n') if self.search.as_ref().is_some_and(|s| !s.matches.is_empty()) => {
                self.search_next();
                DataGridAction::None
            }
            KeyCode::Char('N') if self.search.as_ref().is_some_and(|s| !s.matches.is_empty()) => {
                self.search_prev();
                DataGridAction::None
            }

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

            // Open full-text search (Ctrl+F, available in all modes)
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search = Some(SearchState {
                    query: String::new(),
                    matches: vec![],
                    current: 0,
                    prompt_open: true,
                });
                DataGridAction::None
            }

            // Filter: open input for selected column
            KeyCode::Char('f') if !self.read_only => {
                self.start_filter();
                DataGridAction::None
            }
            // Remove filter on selected column
            KeyCode::Char('d') if !self.read_only => {
                if let Some(name) = self.selected_col_name()
                    && self.filters.remove(&name).is_some() {
                        return DataGridAction::ApplyFilter;
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

            KeyCode::Enter if !self.read_only || self.selected_value_is_nested() => DataGridAction::EnterCell,

            // Manual column resize
            // Export prompt
            KeyCode::Char('E') => {
                if self.result.is_some() {
                    self.export_prompt = true;
                }
                DataGridAction::None
            }

            KeyCode::Char('-') => {
                let current = self.effective_col_width(self.selected_col);
                self.col_widths.insert(
                    self.selected_col,
                    current.saturating_sub(COL_RESIZE_STEP).max(4),
                );
                DataGridAction::None
            }
            KeyCode::Char('=') => {
                let current = self.effective_col_width(self.selected_col);
                self.col_widths.insert(
                    self.selected_col,
                    (current + COL_RESIZE_STEP).min(80),
                );
                DataGridAction::None
            }

            // Sort: cycle None → ASC → DESC → None on selected column
            KeyCode::Char('s') if self.sortable => {
                if let Some(col_name) = self.selected_col_name() {
                    if self.sort_col_name.as_deref() == Some(col_name.as_str()) {
                        if self.sort_asc {
                            self.sort_asc = false; // ASC → DESC
                        } else {
                            self.sort_col_name = None; // DESC → no sort
                        }
                    } else {
                        self.sort_col_name = Some(col_name); // new column → ASC
                        self.sort_asc = true;
                    }
                }
                DataGridAction::ApplySort
            }

            // Load all remaining rows at once
            KeyCode::Char('A') if self.sortable && self.has_more && !self.loading => {
                DataGridAction::LoadAll
            }

            KeyCode::Char('a') if self.is_nosql && !self.read_only && !self.prod_readonly => {
                DataGridAction::InsertMongo
            }
            KeyCode::Char('D') if self.is_nosql && !self.read_only && !self.prod_readonly => {
                DataGridAction::DeleteMongo
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

    pub fn selected_value_is_nested(&self) -> bool {
        let Some(result) = &self.result else { return false };
        let Some(sel_row) = self.table_state.selected() else { return false };
        matches!(
            result.rows.get(sel_row).and_then(|r| r.values.get(self.selected_col)),
            Some(Value::NestedDoc(_)) | Some(Value::NestedArray(_))
        )
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

    // ── Search ────────────────────────────────────────────────────────────────

    fn handle_search_key(&mut self, key: KeyEvent) -> DataGridAction {
        match key.code {
            KeyCode::Esc => {
                self.search = None;
            }
            KeyCode::Enter => {
                if let Some(s) = self.search.as_mut() {
                    s.prompt_open = false;
                }
            }
            KeyCode::Char('n') | KeyCode::Down => self.search_next(),
            KeyCode::Char('N') | KeyCode::Up   => self.search_prev(),
            KeyCode::Backspace => {
                if let Some(s) = self.search.as_mut() { s.query.pop(); }
                self.recompute_search();
            }
            KeyCode::Char(c) => {
                if let Some(s) = self.search.as_mut() { s.query.push(c); }
                self.recompute_search();
            }
            _ => {}
        }
        DataGridAction::None
    }

    fn recompute_search(&mut self) {
        let query = match self.search.as_ref() {
            Some(s) => s.query.to_lowercase(),
            None => return,
        };
        let current_row = self.table_state.selected().unwrap_or(0);
        let current_col = self.selected_col;

        let matches: Vec<(usize, usize)> = if query.is_empty() {
            vec![]
        } else if let Some(result) = &self.result {
            let mut m = vec![];
            for (ri, row) in result.rows.iter().enumerate() {
                for (ci, val) in row.values.iter().enumerate() {
                    if value_display(val).to_lowercase().contains(&query) {
                        m.push((ri, ci));
                    }
                }
            }
            m
        } else {
            vec![]
        };

        let new_current = matches.iter().position(|&(r, c)| {
            r > current_row || (r == current_row && c >= current_col)
        }).unwrap_or(0);

        if let Some(s) = self.search.as_mut() {
            s.matches = matches;
            s.current = new_current;
        }
        self.jump_to_current_match();
    }

    fn jump_to_current_match(&mut self) {
        let pos = self.search.as_ref()
            .and_then(|s| s.matches.get(s.current).copied());
        if let Some((row, col)) = pos {
            self.table_state.select(Some(row));
            self.selected_col = col;
        }
    }

    fn search_next(&mut self) {
        if let Some(s) = self.search.as_mut()
            && !s.matches.is_empty()
        {
            s.current = (s.current + 1) % s.matches.len();
        }
        self.jump_to_current_match();
    }

    fn search_prev(&mut self) {
        if let Some(s) = self.search.as_mut()
            && !s.matches.is_empty()
        {
            let len = s.matches.len();
            s.current = (s.current + len - 1) % len;
        }
        self.jump_to_current_match();
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
                        .style(Style::default().fg(Color::Gray)),
                    chunks[1],
                );
            }
            Some(result) if result.rows.is_empty() => {
                f.render_widget(
                    Paragraph::new("Table is empty (or no rows match the current filters)")
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Gray)),
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
                        let is_sorted = screen.sort_col_name.as_deref() == Some(col_name.as_str());
                        let sort_indicator = if is_sorted {
                            if screen.sort_asc { " ▲" } else { " ▼" }
                        } else {
                            ""
                        };
                        let name = if screen.collapsed_cols.contains(&i) {
                            "…".to_string()
                        } else {
                            let full = format!("{}{}", col_name, sort_indicator);
                            truncate(&full, screen.effective_col_width(i) as usize)
                        };
                        // Selected + filtered → yellow underlined
                        // Selected only       → yellow underlined
                        // Filtered only       → cyan bold
                        // Sorted              → green bold (if not selected)
                        // Normal              → bold
                        let style = if i == screen.selected_col {
                            let base = Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                            if is_filtered { base.bg(Color::DarkGray) } else { base }
                        } else if is_filtered {
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                        } else if is_sorted {
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
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

                // Pre-compute search match set for O(1) per-cell lookup
                let search_match_set: HashSet<(usize, usize)> = screen.search.as_ref()
                    .filter(|s| !s.matches.is_empty())
                    .map(|s| s.matches.iter().cloned().collect())
                    .unwrap_or_default();

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
                                } else if matches!(val, Value::NestedDoc(_)) {
                                    (String::new(), " [obj]".to_string())
                                } else if matches!(val, Value::NestedArray(_)) {
                                    (String::new(), format!(" {}", s))
                                } else if let Some(tbl) = fk_table {
                                    // Always show the FK badge; truncate the value to whatever
                                    // space remains (can be 0 if the badge fills the column).
                                    let badge = format!(" [{}]", tbl);
                                    let avail = col_width.saturating_sub(badge.len());
                                    (truncate(&s, avail), badge)
                                } else {
                                    (truncate(&s, col_width), String::new())
                                };

                                let is_search_match = search_match_set.contains(&(row_idx, col_idx));
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
                                } else if is_search_match {
                                    Style::default().fg(Color::Black).bg(Color::Green)
                                } else if matches!(val, Value::Null) {
                                    Style::default().fg(Color::Gray)
                                } else {
                                    Style::default()
                                };

                                let badge_style = if matches!(val, Value::NestedDoc(_) | Value::NestedArray(_)) {
                                    Style::default().fg(Color::Green)
                                } else {
                                    Style::default().fg(Color::Magenta)
                                };
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
            let display = match val {
                Value::NestedDoc(s) | Value::NestedArray(s) => s.as_str(),
                _ => return Some(format!(" ▸ {} : {}", col_name, value_display(val))),
            };
            Some(format!(" ▸ {} : {}", col_name, display))
        }).unwrap_or_default();
        f.render_widget(
            Paragraph::new(preview_text)
                .style(Style::default().fg(Color::White).bg(Color::DarkGray)),
            chunks[2],
        );

        // ── Help / filter / search bar ────────────────────────────────────────
        if screen.export_prompt {
            f.render_widget(
                Paragraph::new(" Export:  c = CSV   j = JSON   J = JSON+FK   Esc = cancel ")
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::Yellow)),
                chunks[3],
            );
        } else if let Some(ref s) = screen.search {
            if s.prompt_open {
                let match_info = if s.query.is_empty() {
                    String::new()
                } else if s.matches.is_empty() {
                    "  no match".to_string()
                } else {
                    format!("  {}/{} matches  —  n/↓: next   N/↑: prev   Enter: keep   Esc: clear",
                        s.current + 1, s.matches.len())
                };
                let prompt = format!(" / {}", s.query);
                let full = format!("{}{}", prompt, match_info);
                let color = if !s.query.is_empty() && s.matches.is_empty() {
                    Color::Red
                } else {
                    Color::Yellow
                };
                f.render_widget(
                    Paragraph::new(full)
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(color)),
                    chunks[3],
                );
                f.set_cursor(chunks[3].x + 1 + prompt.len() as u16, chunks[3].y + 1);
            } else {
                // Nav mode: prompt closed, matches still highlighted
                let info = format!(
                    " / \"{}\"   {}/{}  —  n: next   N: prev   Esc: clear search",
                    s.query, s.current + 1, s.matches.len()
                );
                f.render_widget(
                    Paragraph::new(info)
                        .block(Block::default().borders(Borders::ALL))
                        .style(Style::default().fg(Color::Cyan)),
                    chunks[3],
                );
            }
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
                    " j/k: rows   h/l: cols   -/=: resize   g/G: first/last   Space: collapse   Enter: explore   Ctrl+F: search   E: export   q: back"
                )
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Gray)),
                chunks[3],
            );
        } else if screen.prod_readonly {
            let filter_hint = if screen.filters.is_empty() {
                "f: filter col"
            } else {
                "f: edit filter   d: rm col filter   F: clear all"
            };
            f.render_widget(
                Paragraph::new(format!(
                    " j/k: rows   h/l: cols   -/=: resize   g/G: first/last   {}   Ctrl+F: search   E: export   q: back   [READ-ONLY]",
                    filter_hint
                ))
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Gray)),
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
            let nosql_hint = if screen.is_nosql { "   a: insert   D: delete" } else { "" };
            let sort_hint = if screen.sortable {
                let load_all = if screen.has_more { "   A: load all" } else { "" };
                format!("   s: sort{}", load_all)
            } else {
                String::new()
            };
            f.render_widget(
                Paragraph::new(format!(
                    " j/k: rows   h/l: cols   -/=: resize   g/G: first/last   Enter: cell   {}   {}{}   Ctrl+F: search   E: export{}   q: back",
                    collapse_label, filter_hint, sort_hint, nosql_hint
                ))
                .block(Block::default().borders(Borders::ALL))
                .style(Style::default().fg(Color::Gray)),
                chunks[3],
            );
        }
    }
}

// ── Formatting helpers ────────────────────────────────────────────────────────

fn format_float(f: f64) -> String {
    let s = format!("{f:.4}");
    if let Some(dot_pos) = s.find('.') {
        let frac = &s[dot_pos + 1..];
        let sig = frac.trim_end_matches('0').len().max(2);
        let display_frac: String = frac.chars()
            .chain(std::iter::repeat('0'))
            .take(sig)
            .collect();
        format!("{}.{}", &s[..dot_pos], display_frac)
    } else {
        s
    }
}

fn value_display(v: &Value) -> String {
    match v {
        Value::Null          => "NULL".into(),
        Value::Bool(b)       => b.to_string(),
        Value::Int(i)        => i.to_string(),
        Value::Float(f)      => format_float(*f),
        Value::Text(s)       => s.replace('\n', "↵").replace('\r', ""),
        Value::Bytes(b)      => format!("<{} bytes>", b.len()),
        Value::NestedDoc(_)  => "{…}".into(),
        Value::NestedArray(s) => {
            let count = serde_json::from_str::<serde_json::Value>(s)
                .ok()
                .and_then(|v| v.as_array().map(|a| a.len()))
                .unwrap_or(0);
            format!("[arr:{count}]")
        }
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
