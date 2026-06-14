use std::collections::BTreeMap;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::{backend::Backend, Terminal};
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{timeout, Duration},
};
use crate::config::{Config, ConnectionProfile};
use crate::db::{connectors, traits::{KvClient, SqlClient}, types::{ColumnSchema, DbQueryResult, Value}};
use crate::ui::screens::connection::{ConnectionAction, ConnectionScreen};
use crate::ui::screens::data_grid::{DataGridAction, DataGridScreen, PAGE_SIZE};
use crate::ui::screens::edit_record::{EditRecordAction, EditRecordScreen};
use crate::ui::screens::sql_editor::{SqlEditorAction, SqlEditorScreen};
use crate::ui::screens::table_list::{TableListAction, TableListScreen};

// ── Active connection ─────────────────────────────────────────────────────────

pub enum ActiveClient {
    Sql(Arc<dyn SqlClient>),
    Kv(Arc<dyn KvClient>),
}

// ── Async DB events ───────────────────────────────────────────────────────────

pub enum DbEvent {
    SqlConnected { client: Arc<dyn SqlClient>, url: String, db_type: String },
    KvConnected  { client: Arc<dyn KvClient>,  url: String, db_type: String },
    ConnectionFailed(String),
    TablesLoaded(Vec<String>),
    TablesLoadFailed(String),
    DataLoaded(DbQueryResult),
    DataPageLoaded(DbQueryResult),
    DataCountLoaded(u64),
    DataLoadFailed(String),
    SchemaLoaded(Vec<ColumnSchema>),
    SchemaLoadFailed(String),
    FkPageLoaded(DbQueryResult),
    FkCountLoaded(u64),
    FkSchemaLoaded(Vec<ColumnSchema>),
    QueryRows(DbQueryResult),
    QueryExecuted(u64),
    QueryFailed(String),
    EditSaved,
    EditFailed(String),
}

// ── App state machine ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Connection,
    TableList,
    DataGrid,
    FkGrid,
    EditRecord,
    SqlEditor,
    Quit,
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub state: AppState,
    pub should_quit: bool,
    pub connection_screen: ConnectionScreen,
    pub table_list_screen: TableListScreen,
    pub data_grid_screen: DataGridScreen,
    pub fk_grid_screen: DataGridScreen,
    pub edit_record_screen: EditRecordScreen,
    pub sql_editor_screen: SqlEditorScreen,
    pub active_client: Option<ActiveClient>,
    // State to return to after EditRecord (DataGrid or FkGrid)
    edit_origin: AppState,
    db_tx: mpsc::Sender<DbEvent>,
    db_rx: mpsc::Receiver<DbEvent>,
}

impl App {
    pub fn new() -> Self {
        let profiles = Config::load().unwrap_or_default().connections;
        let (db_tx, db_rx) = mpsc::channel(16);
        Self {
            state: AppState::Connection,
            should_quit: false,
            connection_screen: ConnectionScreen::new(profiles),
            table_list_screen: TableListScreen::new(),
            data_grid_screen: DataGridScreen::new(String::new()),
            fk_grid_screen: DataGridScreen::new(String::new()),
            edit_record_screen: EditRecordScreen::new(String::new(), vec![], vec![]),
            sql_editor_screen: SqlEditorScreen::new(String::new()),
            active_client: None,
            edit_origin: AppState::DataGrid,
            db_tx,
            db_rx,
        }
    }

    pub async fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut events = EventStream::new();

        loop {
            while let Ok(ev) = self.db_rx.try_recv() {
                self.handle_db_event(ev);
            }

            terminal.draw(|f| crate::ui::layout::draw(f, self))?;

            if self.should_quit {
                break;
            }

            if let Ok(Some(Ok(Event::Key(key)))) =
                timeout(Duration::from_millis(50), events.next()).await
            {
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.should_quit = true;
                } else {
                    self.handle_key(key);
                }
            }
        }

        Ok(())
    }

    // ── Key dispatch ──────────────────────────────────────────────────────────

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        match self.state {
            AppState::Connection => {
                match self.connection_screen.handle_key(key) {
                    ConnectionAction::Quit => self.should_quit = true,
                    ConnectionAction::Connect { url, db_type } => {
                        self.spawn_connect(url, db_type);
                    }
                    ConnectionAction::DeleteProfile { idx, persist } => {
                        if idx < self.connection_screen.profiles.len() {
                            let profile = self.connection_screen.profiles.remove(idx);
                            let len = self.connection_screen.profiles.len();
                            if len == 0 {
                                self.connection_screen.list_state.select(None);
                            } else {
                                self.connection_screen.list_state.select(Some(idx.min(len - 1)));
                            }
                            if persist {
                                match Config::delete_profile(&profile.url) {
                                    Ok(()) => {
                                        self.connection_screen.status =
                                            Some(format!("Deleted \"{}\"", profile.name));
                                    }
                                    Err(e) => {
                                        self.connection_screen.status =
                                            Some(format!("Error deleting: {e}"));
                                    }
                                }
                            }
                        }
                    }
                    ConnectionAction::SaveProfile { name, url, db_type } => {
                        let profile = ConnectionProfile { name: name.clone(), db_type, url };
                        match Config::save_profile(profile) {
                            Ok(()) => {
                                let profiles = Config::load().unwrap_or_default().connections;
                                let idx = profiles.iter().position(|p| p.name == name);
                                self.connection_screen.profiles = profiles;
                                if let Some(i) = idx {
                                    self.connection_screen.list_state.select(Some(i));
                                }
                                self.connection_screen.status =
                                    Some(format!("Saved \"{name}\""));
                            }
                            Err(e) => {
                                self.connection_screen.status =
                                    Some(format!("Error saving: {e}"));
                            }
                        }
                    }
                    ConnectionAction::None => {}
                }
            }
            AppState::TableList => {
                match self.table_list_screen.handle_key(key) {
                    TableListAction::OpenTable(name) => self.spawn_load_data(name),
                    TableListAction::OpenEditor => self.open_sql_editor(),
                    TableListAction::Disconnect => {
                        self.active_client = None;
                        self.table_list_screen = TableListScreen::new();
                        self.connection_screen.reset_input();
                        self.state = AppState::Connection;
                    }
                    TableListAction::None => {}
                }
            }
            AppState::DataGrid => {
                match self.data_grid_screen.handle_key(key) {
                    DataGridAction::Back => self.state = AppState::TableList,
                    DataGridAction::ApplyFilter => self.spawn_reload_filters(),
                    DataGridAction::LoadMore => self.spawn_load_more(),
                    DataGridAction::EnterCell => {
                        if let Some((ref_table, ref_col, fk_val)) = self.selected_fk_info(false) {
                            self.open_fk_subgrid(ref_table, ref_col, fk_val);
                        } else {
                            self.open_edit_record();
                        }
                    }
                    DataGridAction::None => {}
                }
            }
            AppState::FkGrid => {
                match self.fk_grid_screen.handle_key(key) {
                    DataGridAction::Back => self.state = AppState::DataGrid,
                    DataGridAction::EnterCell => self.open_fk_edit_record(),
                    DataGridAction::LoadMore | DataGridAction::ApplyFilter => {}
                    DataGridAction::None => {}
                }
            }
            AppState::EditRecord => {
                match self.edit_record_screen.handle_key(key) {
                    EditRecordAction::Back => self.state = self.edit_origin.clone(),
                    EditRecordAction::Save(sql) => self.spawn_save_record(sql),
                    EditRecordAction::None => {}
                }
            }
            AppState::SqlEditor => {
                match self.sql_editor_screen.handle_key(key) {
                    SqlEditorAction::Execute(sql) => self.spawn_execute_query(sql),
                    SqlEditorAction::Back => self.state = AppState::TableList,
                    SqlEditorAction::None => {}
                }
            }
            _ => {
                if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
        }
    }

    // ── Edit record ───────────────────────────────────────────────────────────

    // Returns (ref_table, ref_col, fk_val) if selected cell is a FK, None otherwise.
    // `from_fk` = true → look in fk_grid_screen, false → data_grid_screen.
    fn selected_fk_info(&self, from_fk: bool) -> Option<(String, String, String)> {
        let screen = if from_fk { &self.fk_grid_screen } else { &self.data_grid_screen };
        let result = screen.result.as_ref()?;
        let schema = screen.schema.as_ref()?;
        let col_idx = screen.selected_col;
        let col_name = result.columns.get(col_idx)?.name.as_str();
        let fk = schema.iter().find(|cs| cs.name == col_name)?.fk.as_ref()?;
        let sel_row = screen.table_state.selected()?;
        let val = result.rows.get(sel_row)?.values.get(col_idx)?;
        if matches!(val, Value::Null) { return None; }
        let fk_val = value_to_string(val);
        if fk_val == "NULL" { return None; }
        Some((fk.table.clone(), fk.column.clone(), fk_val))
    }

    fn open_fk_subgrid(&mut self, ref_table: String, ref_col: String, fk_val: String) {
        let display = format!("{} [{}={}]", ref_table, ref_col, fk_val);
        self.fk_grid_screen = DataGridScreen::new(display);
        self.state = AppState::FkGrid;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            let tx = self.db_tx.clone();
            let client = Arc::clone(c);
            let query = build_fk_query(&ref_table, &ref_col, &fk_val);
            tokio::spawn(async move {
                let ev = match client.fetch_all(&query).await {
                    Ok(r)  => DbEvent::FkPageLoaded(r),
                    Err(e) => DbEvent::DataLoadFailed(e.to_string()),
                };
                let _ = tx.send(ev).await;
            });

            let tx = self.db_tx.clone();
            let client = Arc::clone(c);
            let cquery = build_fk_count_query(&ref_table, &ref_col, &fk_val);
            tokio::spawn(async move {
                if let Ok(r) = client.fetch_all(&cquery).await {
                    let _ = tx.send(DbEvent::FkCountLoaded(parse_count(&r))).await;
                }
            });

            let tx = self.db_tx.clone();
            let client = Arc::clone(c);
            tokio::spawn(async move {
                if let Ok(s) = client.get_schema(&ref_table).await {
                    let _ = tx.send(DbEvent::FkSchemaLoaded(s)).await;
                }
            });
        }
    }

    fn open_fk_edit_record(&mut self) {
        let result = match self.fk_grid_screen.result.as_ref() {
            Some(r) => r,
            None => return,
        };
        let schema = match self.fk_grid_screen.schema.as_ref() {
            Some(s) => s.clone(),
            None => {
                self.fk_grid_screen.status = Some("Schema loading, please wait…".into());
                return;
            }
        };
        let sel_row = self.fk_grid_screen.table_state.selected().unwrap_or(0);
        let row = match result.rows.get(sel_row) {
            Some(r) => r,
            None => return,
        };
        let table_name = self.fk_grid_screen.table_name.clone();
        let values: Vec<String> = schema.iter().map(|col| {
            result.columns.iter().position(|c| c.name == col.name)
                .and_then(|i| row.values.get(i))
                .map(value_to_string)
                .unwrap_or_else(|| "NULL".into())
        }).collect();
        self.edit_record_screen = EditRecordScreen::new(table_name, schema, values);
        self.edit_origin = AppState::FkGrid;
        self.state = AppState::EditRecord;
    }

    fn open_edit_record(&mut self) {
        let result = match self.data_grid_screen.result.as_ref() {
            Some(r) => r,
            None => return,
        };
        let schema = match self.data_grid_screen.schema.as_ref() {
            Some(s) => s.clone(),
            None => {
                self.data_grid_screen.status = Some("Schema loading, please wait…".into());
                return;
            }
        };
        let sel_row = self.data_grid_screen.table_state.selected().unwrap_or(0);
        let row = match result.rows.get(sel_row) {
            Some(r) => r,
            None => return,
        };
        let table_name = self.data_grid_screen.table_name.clone();
        let values: Vec<String> = schema.iter().map(|col| {
            result.columns.iter().position(|c| c.name == col.name)
                .and_then(|i| row.values.get(i))
                .map(value_to_string)
                .unwrap_or_else(|| "NULL".into())
        }).collect();
        self.edit_record_screen = EditRecordScreen::new(table_name, schema, values);
        self.edit_origin = AppState::DataGrid;
        self.state = AppState::EditRecord;
    }

    fn spawn_save_record(&mut self, sql: String) {
        self.edit_record_screen.status = Some("Saving…".into());
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::Sql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.execute(&sql).await {
                        Ok(_)  => DbEvent::EditSaved,
                        Err(e) => DbEvent::EditFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
            }
            _ => {
                self.edit_record_screen.set_error("Not connected to a SQL database".into());
            }
        }
    }

    // ── SQL editor ────────────────────────────────────────────────────────────

    fn open_sql_editor(&mut self) {
        let db_info = self.table_list_screen.db_info.clone();
        self.sql_editor_screen = SqlEditorScreen::new(db_info);
        self.state = AppState::SqlEditor;
    }

    // ── Async connection ──────────────────────────────────────────────────────

    fn spawn_connect(&mut self, url: String, db_type: String) {
        self.connection_screen.status =
            Some(format!("Connecting [{db_type}] {url} …"));
        let tx = self.db_tx.clone();
        tokio::spawn(async move {
            let event = if db_type == "redis" {
                match connectors::connect_kv(&db_type, &url).await {
                    Ok(c)  => DbEvent::KvConnected { client: Arc::from(c), url, db_type },
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            } else {
                match connectors::connect_sql(&db_type, &url).await {
                    Ok(c)  => DbEvent::SqlConnected { client: Arc::from(c), url, db_type },
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            };
            let _ = tx.send(event).await;
        });
    }

    // ── Async table listing ───────────────────────────────────────────────────

    fn spawn_load_tables(&mut self) {
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::Sql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.get_tables().await {
                        Ok(t)  => DbEvent::TablesLoaded(t),
                        Err(e) => DbEvent::TablesLoadFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
            }
            Some(ActiveClient::Kv(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.keys("*").await {
                        Ok(k)  => DbEvent::TablesLoaded(k),
                        Err(e) => DbEvent::TablesLoadFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
            }
            None => {}
        }
    }

    // ── Async data loading ────────────────────────────────────────────────────

    fn spawn_load_data(&mut self, table_name: String) {
        self.data_grid_screen = DataGridScreen::new(table_name.clone());
        self.state = AppState::DataGrid;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            let empty = BTreeMap::new();
            spawn_sql_page(Arc::clone(c), &table_name, &empty, 0, true, self.db_tx.clone());
            spawn_sql_count(Arc::clone(c), &table_name, &empty, self.db_tx.clone());
            let tx = self.db_tx.clone();
            let client = Arc::clone(c);
            tokio::spawn(async move {
                let ev = match client.get_schema(&table_name).await {
                    Ok(s)  => DbEvent::SchemaLoaded(s),
                    Err(e) => DbEvent::SchemaLoadFailed(e.to_string()),
                };
                let _ = tx.send(ev).await;
            });
        } else if let Some(ActiveClient::Kv(_)) = &self.active_client {
            self.data_grid_screen.set_error(
                "Data Grid not available for key-value stores. Use the SQL Editor.".into(),
            );
        } else {
            self.data_grid_screen.set_error("Not connected".into());
        }
    }

    // Load next page (infinite scroll — triggered when user reaches last row)
    fn spawn_load_more(&mut self) {
        if self.data_grid_screen.loading { return; }
        self.data_grid_screen.loading = true;

        let table   = self.data_grid_screen.table_name.clone();
        let filters = self.data_grid_screen.filters.clone();
        let offset  = self.data_grid_screen.loaded_count;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            spawn_sql_page(Arc::clone(c), &table, &filters, offset, false, self.db_tx.clone());
        }
    }

    // Re-fetch from page 0 with the current filters (after filter add/remove)
    fn spawn_reload_filters(&mut self) {
        let table   = self.data_grid_screen.table_name.clone();
        let filters = self.data_grid_screen.filters.clone();
        self.data_grid_screen.reset_data(); // keeps table_name + filters
        // reset_data sets loading=true but doesn't spawn — we do it here
        self.data_grid_screen.total_count = None;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            spawn_sql_page(Arc::clone(c), &table, &filters, 0, true, self.db_tx.clone());
            spawn_sql_count(Arc::clone(c), &table, &filters, self.db_tx.clone());
        } else {
            self.data_grid_screen.set_error("Not connected to a SQL database".into());
        }
    }

    // ── Async SQL execution (SQL editor) ──────────────────────────────────────

    fn spawn_execute_query(&mut self, sql: String) {
        self.sql_editor_screen.set_running();
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::Sql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = if is_select_query(&sql) {
                        match c.fetch_all(&sql).await {
                            Ok(r)  => DbEvent::QueryRows(r),
                            Err(e) => DbEvent::QueryFailed(e.to_string()),
                        }
                    } else {
                        match c.execute(&sql).await {
                            Ok(n)  => DbEvent::QueryExecuted(n),
                            Err(e) => DbEvent::QueryFailed(e.to_string()),
                        }
                    };
                    let _ = tx.send(ev).await;
                });
            }
            Some(ActiveClient::Kv(_)) => {
                self.sql_editor_screen.set_error(
                    "SQL editor requires a SQL connection (not Redis)".into(),
                );
            }
            None => {
                self.sql_editor_screen.set_error("Not connected".into());
            }
        }
    }

    // ── DB event handler ──────────────────────────────────────────────────────

    fn handle_db_event(&mut self, event: DbEvent) {
        match event {
            DbEvent::SqlConnected { client, url, db_type } => {
                self.active_client = Some(ActiveClient::Sql(client));
                self.connection_screen.status = None;
                self.table_list_screen = TableListScreen::new();
                self.table_list_screen.db_info = format!("[{db_type}] {url}");
                self.state = AppState::TableList;
                self.spawn_load_tables();
            }
            DbEvent::KvConnected { client, url, db_type } => {
                self.active_client = Some(ActiveClient::Kv(client));
                self.connection_screen.status = None;
                self.table_list_screen = TableListScreen::new();
                self.table_list_screen.db_info = format!("[{db_type}] {url}");
                self.state = AppState::TableList;
                self.spawn_load_tables();
            }
            DbEvent::ConnectionFailed(msg) => {
                self.connection_screen.status = Some(format!("Error: {msg}"));
            }
            DbEvent::TablesLoaded(tables) => {
                self.table_list_screen.set_tables(tables);
            }
            DbEvent::TablesLoadFailed(msg) => {
                self.table_list_screen.set_error(format!("Error: {msg}"));
            }
            DbEvent::DataLoaded(result) => {
                self.data_grid_screen.set_result(result);
            }
            DbEvent::DataPageLoaded(result) => {
                self.data_grid_screen.append_rows(result);
            }
            DbEvent::DataCountLoaded(n) => {
                self.data_grid_screen.set_total(n);
            }
            DbEvent::DataLoadFailed(msg) => {
                self.data_grid_screen.set_error(msg);
            }
            DbEvent::SchemaLoaded(schema) => {
                self.data_grid_screen.schema = Some(schema);
            }
            DbEvent::SchemaLoadFailed(_) => {
                // Non-fatal: schema is optional for viewing; edit will show a warning
            }
            DbEvent::FkPageLoaded(result) => {
                self.fk_grid_screen.set_result(result);
            }
            DbEvent::FkCountLoaded(n) => {
                self.fk_grid_screen.set_total(n);
            }
            DbEvent::FkSchemaLoaded(schema) => {
                self.fk_grid_screen.schema = Some(schema);
            }
            DbEvent::QueryRows(result) => {
                self.sql_editor_screen.set_rows(result);
            }
            DbEvent::QueryExecuted(n) => {
                self.sql_editor_screen.set_affected(n);
            }
            DbEvent::QueryFailed(msg) => {
                self.sql_editor_screen.set_error(msg);
            }
            DbEvent::EditSaved => {
                self.state = AppState::DataGrid;
                let table   = self.data_grid_screen.table_name.clone();
                let filters = self.data_grid_screen.filters.clone();
                self.data_grid_screen.reset_data();
                self.data_grid_screen.total_count = None;
                if let Some(ActiveClient::Sql(c)) = &self.active_client {
                    spawn_sql_page(Arc::clone(c), &table, &filters, 0, true, self.db_tx.clone());
                    spawn_sql_count(Arc::clone(c), &table, &filters, self.db_tx.clone());
                }
            }
            DbEvent::EditFailed(msg) => {
                self.edit_record_screen.set_error(msg);
            }
        }
    }
}

// ── Value display helpers ─────────────────────────────────────────────────────

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null     => "NULL".into(),
        Value::Bool(b)  => b.to_string(),
        Value::Int(i)   => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Text(s)  => s.clone(),
        Value::Bytes(b) => format!("<{} bytes>", b.len()),
    }
}

// ── SQL query helpers ─────────────────────────────────────────────────────────

fn is_select_query(sql: &str) -> bool {
    let upper = sql.trim_start().to_uppercase();
    upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("EXPLAIN")
        || upper.starts_with("SHOW")
        || upper.starts_with("DESCRIBE")
        || upper.starts_with("PRAGMA")
}

// ── Data grid query builders ──────────────────────────────────────────────────

fn build_where(filters: &BTreeMap<String, String>) -> String {
    if filters.is_empty() { return String::new(); }
    let clauses: Vec<String> = filters.iter()
        .map(|(col, val)| {
            let escaped = val.replace('\'', "''");
            format!("\"{}\" LIKE '%{}%'", col, escaped)
        })
        .collect();
    format!(" WHERE {}", clauses.join(" AND "))
}

fn build_data_query(table: &str, filters: &BTreeMap<String, String>, offset: usize) -> String {
    let wh = build_where(filters);
    format!("SELECT * FROM \"{table}\"{wh} LIMIT {PAGE_SIZE} OFFSET {offset}")
}

fn build_count_query(table: &str, filters: &BTreeMap<String, String>) -> String {
    let wh = build_where(filters);
    format!("SELECT COUNT(*) AS _count FROM \"{table}\"{wh}")
}

fn build_fk_query(ref_table: &str, ref_col: &str, fk_val: &str) -> String {
    let safe_t = ref_table.replace('"', "");
    let safe_c = ref_col.replace('"', "");
    let safe_v = fk_val.replace('\'', "''");
    format!("SELECT * FROM \"{safe_t}\" WHERE \"{safe_c}\" = '{safe_v}' LIMIT {PAGE_SIZE}")
}

fn build_fk_count_query(ref_table: &str, ref_col: &str, fk_val: &str) -> String {
    let safe_t = ref_table.replace('"', "");
    let safe_c = ref_col.replace('"', "");
    let safe_v = fk_val.replace('\'', "''");
    format!("SELECT COUNT(*) AS _count FROM \"{safe_t}\" WHERE \"{safe_c}\" = '{safe_v}'")
}

fn parse_count(result: &DbQueryResult) -> u64 {
    result.rows.first()
        .and_then(|r| r.values.first())
        .map(|v| match v {
            Value::Int(n)  => *n as u64,
            Value::Text(s) => s.parse().unwrap_or(0),
            _              => 0,
        })
        .unwrap_or(0)
}

// ── Async spawn helpers ───────────────────────────────────────────────────────

fn spawn_sql_page(
    client: Arc<dyn SqlClient>,
    table: &str,
    filters: &BTreeMap<String, String>,
    offset: usize,
    initial: bool,
    tx: mpsc::Sender<DbEvent>,
) {
    let query = build_data_query(table, filters, offset);
    tokio::spawn(async move {
        let ev = match client.fetch_all(&query).await {
            Ok(r)  => if initial { DbEvent::DataLoaded(r) } else { DbEvent::DataPageLoaded(r) },
            Err(e) => DbEvent::DataLoadFailed(e.to_string()),
        };
        let _ = tx.send(ev).await;
    });
}

fn spawn_sql_count(
    client: Arc<dyn SqlClient>,
    table: &str,
    filters: &BTreeMap<String, String>,
    tx: mpsc::Sender<DbEvent>,
) {
    let query = build_count_query(table, filters);
    tokio::spawn(async move {
        if let Ok(r) = client.fetch_all(&query).await {
            let _ = tx.send(DbEvent::DataCountLoaded(parse_count(&r))).await;
        }
    });
}
