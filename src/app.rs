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
use crate::export;
use crate::history::QueryHistory;
use crate::db::{connectors, traits::{KvClient, NoSqlClient, SqlClient}, types::{Column, ColumnSchema, DbQueryResult, KvKeyDetail, Row, TableObject, Value}};
use crate::ui::components::modal::Modal;
use crate::ui::screens::connection::{ConnectionAction, ConnectionScreen};
use crate::ui::screens::data_grid::{DataGridAction, DataGridScreen, PAGE_SIZE};
use crate::ui::screens::edit_record::{EditRecordAction, EditRecordScreen};
use crate::ui::screens::erd_graph::{ErdGraphAction, ErdGraphScreen};
use crate::ui::screens::sql_editor::{SqlEditorAction, SqlEditorScreen};
use crate::ui::screens::table_list::{TableListAction, TableListScreen};

// ── Active connection ─────────────────────────────────────────────────────────

pub enum ActiveClient {
    Sql(Arc<dyn SqlClient>),
    Kv(Arc<dyn KvClient>),
    NoSql(Arc<dyn NoSqlClient>),
}

// ── Async DB events ───────────────────────────────────────────────────────────

pub enum DbEvent {
    SqlConnected   { client: Arc<dyn SqlClient>,   url: String, db_type: String },
    KvConnected    { client: Arc<dyn KvClient>,    url: String, db_type: String },
    NoSqlConnected { client: Arc<dyn NoSqlClient>, url: String, db_type: String },
    ConnectionFailed(String),
    TablesLoaded(Vec<String>),       // KV stores (Redis key names)
    TableObjectsLoaded(Vec<TableObject>), // SQL: TABLE + VIEW with kind
    TablesLoadFailed(String),
    DataLoaded(DbQueryResult),
    DataPageLoaded(DbQueryResult),
    DataCountLoaded(u64),
    DataLoadFailed(String),
    SchemaLoaded(Vec<ColumnSchema>),
    SchemaLoadFailed,
    FkPageLoaded(DbQueryResult),
    FkCountLoaded(u64),
    FkSchemaLoaded(Vec<ColumnSchema>),
    QueryRows(DbQueryResult),
    QueryExecuted(u64),
    QueryFailed(String),
    EditSaved,
    EditFailed(String),
    ExportDone(std::path::PathBuf),
    ExportFailed(String),
    KvKeyLoaded { detail: KvKeyDetail, key: String, ttl: i64 },
    AllSchemasLoaded(std::collections::HashMap<String, Vec<ColumnSchema>>),
    StatusUpdate(String),
}

// ── Pending modal action ──────────────────────────────────────────────────────

pub enum PendingAction {
    SaveRecord(String),
    SaveMongoRecord { collection: String, id: String, doc_json: String },
    InsertMongoRecord { collection: String, doc_json: String },
    DeleteMongoRecord { collection: String, id: String },
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
    SqlResultGrid,
    ErdGraph,
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub state: AppState,
    pub should_quit: bool,
    pub connection_screen: ConnectionScreen,
    pub table_list_screen: TableListScreen,
    pub data_grid_screen: DataGridScreen,
    pub fk_grid_screen: DataGridScreen,      // current FK level
    fk_history: Vec<DataGridScreen>,         // previous FK levels (navigation stack)
    pub sql_result_grid_screen: DataGridScreen, // read-only grid for SQL Editor results
    pub edit_record_screen: EditRecordScreen,
    edit_record_stack: Vec<(EditRecordScreen, usize)>, // nested object editing stack
    pub erd_graph_screen: ErdGraphScreen,
    pub sql_editor_screen: SqlEditorScreen,
    pub active_client: Option<ActiveClient>,
    pub connected_db_info: Option<String>,
    pub prod_readonly: bool,
    post_disconnect_script: Option<String>,
    pub modal: Option<Modal>,
    pending_action: Option<PendingAction>,
    pub status_message: Option<(String, bool)>,
    pub status_message_ttl: u8,
    pub history: QueryHistory,
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
            fk_history: Vec::new(),
            sql_result_grid_screen: DataGridScreen::new(String::new()),
            edit_record_screen: EditRecordScreen::new(String::new(), vec![], vec![]),
            edit_record_stack: Vec::new(),
            erd_graph_screen: ErdGraphScreen::new(String::new(), std::collections::HashMap::new()),
            sql_editor_screen: SqlEditorScreen::new(String::new()),
            active_client: None,
            connected_db_info: None,
            prod_readonly: false,
            post_disconnect_script: None,
            modal: None,
            pending_action: None,
            status_message: None,
            status_message_ttl: 0,
            history: QueryHistory::load(),
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
                self.run_post_disconnect().await;
                break;
            }

            if let Ok(Some(Ok(Event::Key(key)))) =
                timeout(Duration::from_millis(50), events.next()).await
            {
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.should_quit = true;
                } else if self.modal.is_some() {
                    self.handle_modal_key(key);
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
                    ConnectionAction::Connect { url, db_type, pre_connect, post_disconnect } => {
                        self.spawn_connect(url, db_type, pre_connect, post_disconnect);
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
                    ConnectionAction::SaveProfile { name, url, db_type, pre_connect, post_disconnect } => {
                        let profile = ConnectionProfile { name: name.clone(), db_type, url, pre_connect, post_disconnect };
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
                    TableListAction::OpenTable { name, is_view } => {
                        self.spawn_load_data(name);
                        // spawn_load_data resets data_grid_screen; set view flags after
                        if is_view {
                            self.data_grid_screen.is_view = true;
                            self.data_grid_screen.prod_readonly = true;
                        }
                    }
                    TableListAction::OpenEditor => self.open_sql_editor(),
                    TableListAction::OpenErd(name) => self.open_erd_graph(name),
                    TableListAction::SelectionChanged => {}
                    TableListAction::Disconnect => {
                        self.spawn_post_disconnect();
                        self.active_client = None;
                        self.prod_readonly = false;
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
                        if let Some((col, json, arr)) = nested_info_from(&self.data_grid_screen) {
                            self.open_nested_subgrid(col, json, arr);
                        } else if self.prod_readonly || self.data_grid_screen.read_only {
                            self.status_message = Some(("Read-only mode — edits disabled".into(), true));
                            self.status_message_ttl = 60;
                        } else if let Some((ref_table, ref_col, fk_val)) = self.selected_fk_info(false) {
                            self.open_fk_subgrid(ref_table, ref_col, fk_val);
                        } else {
                            self.open_edit_record();
                        }
                    }
                    DataGridAction::ExportCsv => {
                        let table = self.data_grid_screen.table_name.clone();
                        if let Some(ref r) = self.data_grid_screen.result.clone() {
                            self.run_export(r, &table, None, false);
                        }
                    }
                    DataGridAction::ExportJson => {
                        let table = self.data_grid_screen.table_name.clone();
                        let schema = self.data_grid_screen.schema.clone();
                        if let Some(ref r) = self.data_grid_screen.result.clone() {
                            self.run_export(r, &table, schema, true);
                        }
                    }
                    DataGridAction::ExportJsonSimple => {
                        let table = self.data_grid_screen.table_name.clone();
                        if let Some(ref r) = self.data_grid_screen.result.clone() {
                            self.run_export(r, &table, None, true);
                        }
                    }
                    DataGridAction::InsertMongo => {
                        self.open_insert_mongo_record();
                    }
                    DataGridAction::DeleteMongo => {
                        if let Some(id) = self.get_current_mongo_id() {
                            let collection = self.data_grid_screen.table_name.clone();
                            self.pending_action = Some(PendingAction::DeleteMongoRecord { collection, id: id.clone() });
                            self.modal = Some(Modal::confirm(
                                "Confirm Delete",
                                &format!("Delete document with _id: {id}?"),
                            ));
                        }
                    }
                    DataGridAction::None => {}
                }
            }
            AppState::FkGrid => {
                match self.fk_grid_screen.handle_key(key) {
                    DataGridAction::Back => {
                        if let Some(prev) = self.fk_history.pop() {
                            self.fk_grid_screen = prev; // go up one FK level
                        } else {
                            self.state = AppState::DataGrid;
                        }
                    }
                    DataGridAction::EnterCell => {
                        if let Some((col, json, arr)) = nested_info_from(&self.fk_grid_screen) {
                            self.open_nested_subgrid(col, json, arr);
                        } else if self.prod_readonly || self.fk_grid_screen.read_only {
                            self.status_message = Some(("Read-only mode — edits disabled".into(), true));
                            self.status_message_ttl = 60;
                        } else if let Some((ref_table, ref_col, fk_val)) = self.selected_fk_info(true) {
                            self.open_fk_subgrid(ref_table, ref_col, fk_val);
                        } else {
                            self.open_fk_edit_record();
                        }
                    }
                    DataGridAction::ExportCsv => {
                        let table = self.fk_grid_screen.table_name.clone();
                        if let Some(ref r) = self.fk_grid_screen.result.clone() {
                            self.run_export(r, &table, None, false);
                        }
                    }
                    DataGridAction::ExportJson => {
                        let table = self.fk_grid_screen.table_name.clone();
                        let schema = self.fk_grid_screen.schema.clone();
                        if let Some(ref r) = self.fk_grid_screen.result.clone() {
                            self.run_export(r, &table, schema, true);
                        }
                    }
                    DataGridAction::ExportJsonSimple => {
                        let table = self.fk_grid_screen.table_name.clone();
                        if let Some(ref r) = self.fk_grid_screen.result.clone() {
                            self.run_export(r, &table, None, true);
                        }
                    }
                    DataGridAction::LoadMore | DataGridAction::ApplyFilter => {}
                    DataGridAction::InsertMongo | DataGridAction::DeleteMongo => {}
                    DataGridAction::None => {}
                }
            }
            AppState::EditRecord => {
                match self.edit_record_screen.handle_key(key) {
                    EditRecordAction::Back => {
                        if !self.edit_record_stack.is_empty() {
                            self.pop_nested_edit_record();
                        } else {
                            self.state = self.edit_origin.clone();
                        }
                    }
                    EditRecordAction::OpenNested(field_idx) => {
                        self.open_nested_edit_record(field_idx);
                    }
                    EditRecordAction::Save(sql) => {
                        let preview: String = sql.chars().take(120).collect();
                        self.pending_action = Some(PendingAction::SaveRecord(sql));
                        self.modal = Some(Modal::confirm(
                            "Confirm Save",
                            &format!("Execute this statement?\n{preview}"),
                        ));
                    }
                    EditRecordAction::SaveMongo { id, doc_json } => {
                        if !self.edit_record_stack.is_empty() {
                            self.edit_record_screen.status = Some("Press Esc to confirm nested edit first".into());
                        } else {
                            let collection = self.data_grid_screen.table_name.clone();
                            let preview: String = doc_json.chars().take(120).collect();
                            self.pending_action = Some(PendingAction::SaveMongoRecord { collection, id, doc_json });
                            self.modal = Some(Modal::confirm(
                                "Confirm Save",
                                &format!("Replace MongoDB document?\n{preview}"),
                            ));
                        }
                    }
                    EditRecordAction::InsertMongo { doc_json } => {
                        if !self.edit_record_stack.is_empty() {
                            self.edit_record_screen.status = Some("Press Esc to confirm nested edit first".into());
                        } else {
                            let collection = self.data_grid_screen.table_name.clone();
                            let preview: String = doc_json.chars().take(120).collect();
                            self.pending_action = Some(PendingAction::InsertMongoRecord { collection, doc_json });
                            self.modal = Some(Modal::confirm(
                                "Confirm Insert",
                                &format!("Insert new MongoDB document?\n{preview}"),
                            ));
                        }
                    }
                    EditRecordAction::None => {}
                }
            }
            AppState::SqlEditor => {
                match self.sql_editor_screen.handle_key(key) {
                    SqlEditorAction::Execute(sql) => {
                        self.history.push(sql.clone());
                        self.spawn_execute_query(sql);
                    }
                    SqlEditorAction::Back => self.state = AppState::TableList,
                    SqlEditorAction::OpenGrid(result) => self.open_sql_result_grid(result),
                    SqlEditorAction::HistoryPrev => {
                        if let Some(entry) = self.history.prev() {
                            self.sql_editor_screen.set_editor_content(entry);
                        }
                    }
                    SqlEditorAction::HistoryNext => {
                        let content = self.history.next();
                        self.sql_editor_screen.set_editor_content(content.unwrap_or(""));
                    }
                    SqlEditorAction::None => {}
                }
            }
            AppState::ErdGraph => {
                match self.erd_graph_screen.handle_key(key) {
                    ErdGraphAction::Back => self.state = AppState::TableList,
                    ErdGraphAction::None => {}
                }
            }
            AppState::SqlResultGrid => {
                match self.sql_result_grid_screen.handle_key(key) {
                    DataGridAction::Back => self.state = AppState::SqlEditor,
                    DataGridAction::ExportCsv => {
                        let table = self.sql_result_grid_screen.table_name.clone();
                        if let Some(ref r) = self.sql_result_grid_screen.result.clone() {
                            self.run_export(r, &table, None, false);
                        }
                    }
                    DataGridAction::ExportJson | DataGridAction::ExportJsonSimple => {
                        let table = self.sql_result_grid_screen.table_name.clone();
                        if let Some(ref r) = self.sql_result_grid_screen.result.clone() {
                            self.run_export(r, &table, None, true);
                        }
                    }
                    _ => {}
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
        // If already navigating FK levels, push the current level onto the history stack.
        if self.state == AppState::FkGrid {
            let prev = std::mem::replace(
                &mut self.fk_grid_screen,
                DataGridScreen::new(String::new()),
            );
            self.fk_history.push(prev);
        }
        let display = format!("{} [{}={}]", ref_table, ref_col, fk_val);
        let mut screen = DataGridScreen::new(ref_table.clone());
        screen.display_name = Some(display);
        screen.prod_readonly = self.prod_readonly;
        self.fk_grid_screen = screen;
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

    fn open_nested_subgrid(&mut self, col_name: String, json: String, is_array: bool) {
        // Capture the breadcrumb label before any move/replace
        let parent_name = if self.state == AppState::FkGrid {
            self.fk_grid_screen.display_name.clone()
                .unwrap_or_else(|| self.fk_grid_screen.table_name.clone())
        } else {
            self.data_grid_screen.display_name.clone()
                .unwrap_or_else(|| self.data_grid_screen.table_name.clone())
        };

        if self.state == AppState::FkGrid {
            let prev = std::mem::replace(&mut self.fk_grid_screen, DataGridScreen::new(String::new()));
            self.fk_history.push(prev);
        }

        let result = json_to_result(&json, is_array);
        let count  = result.rows.len();

        let mut screen = DataGridScreen::new(col_name.clone());
        screen.display_name  = Some(format!("{} › {}", parent_name, col_name));
        screen.read_only     = true;
        screen.has_more      = false;
        screen.loading       = false;
        screen.total_count   = Some(count as u64);
        screen.loaded_count  = count;
        if count > 0 {
            screen.table_state.select(Some(0));
        }
        screen.result = Some(result);

        self.fk_grid_screen = screen;
        self.state = AppState::FkGrid;
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
        let sel_row = self.data_grid_screen.table_state.selected().unwrap_or(0);
        let row = match result.rows.get(sel_row) {
            Some(r) => r,
            None => return,
        };
        let table_name = self.data_grid_screen.table_name.clone();

        if matches!(self.active_client, Some(ActiveClient::NoSql(_))) {
            self.edit_record_stack.clear();
            let schema: Vec<ColumnSchema> = result.columns.iter().zip(row.values.iter())
                .map(|(col, val)| ColumnSchema {
                    name: col.name.clone(),
                    type_name: mongo_type_name(val),
                    is_pk: col.name == "_id",
                    is_nullable: true,
                    fk: None,
                })
                .collect();
            let values: Vec<String> = row.values.iter().map(value_to_string).collect();
            let mut screen = EditRecordScreen::new(table_name, schema, values);
            screen.is_nosql = true;
            self.edit_record_screen = screen;
            self.edit_origin = AppState::DataGrid;
            self.state = AppState::EditRecord;
            return;
        }

        let schema = match self.data_grid_screen.schema.as_ref() {
            Some(s) => s.clone(),
            None => {
                self.data_grid_screen.status = Some("Schema loading, please wait…".into());
                return;
            }
        };
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

    fn spawn_save_mongo_record(&mut self, collection: String, id: String, doc_json: String) {
        self.edit_record_screen.status = Some("Saving…".into());
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::NoSql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.replace_one(&collection, &id, &doc_json).await {
                        Ok(n) if n > 0 => DbEvent::EditSaved,
                        Ok(_) => DbEvent::EditFailed("No document matched that _id".into()),
                        Err(e) => DbEvent::EditFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
            }
            _ => {
                self.edit_record_screen.set_error("Not connected to MongoDB".into());
            }
        }
    }

    fn open_insert_mongo_record(&mut self) {
        let result = match self.data_grid_screen.result.as_ref() {
            Some(r) => r,
            None => return,
        };
        let table_name = self.data_grid_screen.table_name.clone();
        // Use first row to infer field types (object/array/int/float/bool/string)
        let first_row = result.rows.first();
        let schema: Vec<ColumnSchema> = result.columns.iter()
            .enumerate()
            .filter(|(_, col)| col.name != "_id")
            .map(|(i, col)| {
                let type_name = first_row
                    .and_then(|r| r.values.get(i))
                    .map(mongo_type_name)
                    .unwrap_or_else(|| "string".to_string());
                ColumnSchema {
                    name: col.name.clone(),
                    type_name,
                    is_pk: false,
                    is_nullable: true,
                    fk: None,
                }
            })
            .collect();
        let values: Vec<String> = schema.iter()
            .map(|col| match col.type_name.as_str() {
                "object" => "{}".to_string(),
                "array"  => "[]".to_string(),
                _        => "".to_string(),
            })
            .collect();
        let mut screen = EditRecordScreen::new(table_name, schema, values);
        screen.is_nosql = true;
        screen.is_insert = true;
        self.edit_record_stack.clear();
        self.edit_record_screen = screen;
        self.edit_origin = AppState::DataGrid;
        self.state = AppState::EditRecord;
    }

    fn get_current_mongo_id(&self) -> Option<String> {
        let result = self.data_grid_screen.result.as_ref()?;
        let sel = self.data_grid_screen.table_state.selected()?;
        let row = result.rows.get(sel)?;
        let id_col_idx = result.columns.iter().position(|c| c.name == "_id")?;
        Some(value_to_string(row.values.get(id_col_idx)?))
    }

    fn spawn_insert_mongo_record(&mut self, collection: String, doc_json: String) {
        self.edit_record_screen.status = Some("Inserting…".into());
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::NoSql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.insert_one(&collection, &doc_json).await {
                        Ok(_)  => DbEvent::EditSaved,
                        Err(e) => DbEvent::EditFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
            }
            _ => {
                self.edit_record_screen.set_error("Not connected to MongoDB".into());
            }
        }
    }

    fn spawn_delete_mongo_record(&mut self, collection: String, id: String) {
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::NoSql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.delete_one(&collection, &id).await {
                        Ok(n) if n > 0 => DbEvent::EditSaved,
                        Ok(_) => DbEvent::EditFailed("No document matched that _id".into()),
                        Err(e) => DbEvent::EditFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
            }
            _ => {
                self.data_grid_screen.status = Some("Not connected to MongoDB".into());
            }
        }
    }

    fn open_nested_edit_record(&mut self, field_idx: usize) {
        let json = self.edit_record_screen.current_values[field_idx].clone();
        let type_name = self.edit_record_screen.schema[field_idx].type_name.clone();
        let is_array = type_name == "array";

        let (schema, values) = if is_array {
            json_array_to_schema_values(&json)
        } else {
            json_object_to_schema_values(&json)
        };

        if !is_array && serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&json).is_err() {
            self.edit_record_screen.status = Some("Cannot parse nested object".into());
            return;
        }
        if is_array && serde_json::from_str::<Vec<serde_json::Value>>(&json).is_err() {
            self.edit_record_screen.status = Some("Cannot parse nested array".into());
            return;
        }

        let field_name = self.edit_record_screen.schema[field_idx].name.clone();
        let parent_title = self.edit_record_screen.table_name.clone();
        let child_title = format!("{parent_title} › {field_name}");
        let mut child = EditRecordScreen::new(child_title, schema, values);
        child.is_nosql = true;
        child.is_array = is_array;
        child.is_nested = true; // sub-editors never save directly; is_array takes priority in preview routing
        let parent = std::mem::replace(&mut self.edit_record_screen, child);
        self.edit_record_stack.push((parent, field_idx));
    }

    fn pop_nested_edit_record(&mut self) {
        if let Some((mut parent, field_idx)) = self.edit_record_stack.pop() {
            let child_json = if self.edit_record_screen.is_array {
                self.edit_record_screen.reconstruct_nested_array()
            } else {
                self.edit_record_screen.reconstruct_nested_json()
            };
            parent.current_values[field_idx] = child_json;
            parent.validation_errors[field_idx] = None;
            self.edit_record_screen = parent;
        }
    }

    // ── SQL editor ────────────────────────────────────────────────────────────

    fn open_sql_editor(&mut self) {
        let db_info = self.table_list_screen.db_info.clone();
        self.sql_editor_screen = SqlEditorScreen::new(db_info);
        if matches!(self.active_client, Some(ActiveClient::NoSql(_))) {
            let coll = self.table_list_screen.selected_table_name();
            self.sql_editor_screen.set_nosql_collection(coll);
        }
        self.state = AppState::SqlEditor;
    }

    fn open_sql_result_grid(&mut self, result: DbQueryResult) {
        let count = result.rows.len();
        let mut screen = DataGridScreen::new(String::new());
        screen.display_name = Some("SQL Result".into());
        screen.read_only = true;
        screen.has_more = false;
        screen.loading = false;
        screen.loaded_count = count;
        screen.total_count = Some(count as u64);
        screen.status = None;
        screen.table_state.select(if count > 0 { Some(0) } else { None });
        screen.result = Some(result);
        self.sql_result_grid_screen = screen;
        self.state = AppState::SqlResultGrid;
    }

    // ── ERD graph (level 2) ───────────────────────────────────────────────────

    fn open_erd_graph(&mut self, name: String) {
        if self.table_list_screen.schemas_loading {
            self.table_list_screen.status = Some("Schema still loading…".into());
            return;
        }
        self.erd_graph_screen = ErdGraphScreen::new(
            name,
            self.table_list_screen.all_schemas.clone(),
        );
        self.state = AppState::ErdGraph;
    }

    // ── Schema preload (for TableList ERD panel) ──────────────────────────────

    fn spawn_load_all_schemas(&mut self) {
        let tx = self.db_tx.clone();
        let tables: Vec<String> = self.table_list_screen.tables.iter()
            .map(|t| t.name.clone())
            .collect();
        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            let c = Arc::clone(c);
            tokio::spawn(async move {
                use std::collections::HashMap;
                let mut schema: HashMap<String, Vec<ColumnSchema>> = HashMap::new();
                for table in &tables {
                    if let Ok(cols) = c.get_schema(table).await {
                        schema.insert(table.clone(), cols);
                    }
                }
                let _ = tx.send(DbEvent::AllSchemasLoaded(schema)).await;
            });
        }
    }

    // ── Modal key handler ─────────────────────────────────────────────────────

    fn handle_modal_key(&mut self, key: crossterm::event::KeyEvent) {
        use crate::ui::components::modal::ModalKind;
        let is_confirm = matches!(self.modal.as_ref().map(|m| &m.kind), Some(ModalKind::Confirm));
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') if is_confirm => {
                self.modal = None;
                if let Some(action) = self.pending_action.take() {
                    match action {
                        PendingAction::SaveRecord(sql) => self.spawn_save_record(sql),
                        PendingAction::SaveMongoRecord { collection, id, doc_json } => {
                            self.spawn_save_mongo_record(collection, id, doc_json);
                        }
                        PendingAction::InsertMongoRecord { collection, doc_json } => {
                            self.spawn_insert_mongo_record(collection, doc_json);
                        }
                        PendingAction::DeleteMongoRecord { collection, id } => {
                            self.spawn_delete_mongo_record(collection, id);
                        }
                    }
                }
            }
            KeyCode::Enter if !is_confirm => {
                // Error modal: Enter closes
                self.modal = None;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.modal = None;
                self.pending_action = None;
            }
            _ => {}
        }
    }

    // ── Async connection ──────────────────────────────────────────────────────

    fn spawn_connect(&mut self, url: String, db_type: String, pre_connect: Option<String>, post_disconnect: Option<String>) {
        let (clean_url, is_readonly) = strip_readonly_param(&url);
        self.prod_readonly = is_readonly;
        self.post_disconnect_script = post_disconnect;

        if pre_connect.is_some() {
            self.connection_screen.status = Some("Running pre-connect script…".into());
        } else {
            self.connection_screen.status =
                Some(format!("Connecting [{db_type}] {} …", redact_url(&clean_url)));
        }

        let tx = self.db_tx.clone();
        tokio::spawn(async move {
            if let Some(script) = pre_connect {
                let _ = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&script)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await;
                let _ = tx.send(DbEvent::StatusUpdate(
                    format!("Connecting [{db_type}] {} …", redact_url(&clean_url))
                )).await;
            }

            let event = if db_type == "redis" {
                match connectors::connect_kv(&db_type, &clean_url).await {
                    Ok(c)  => DbEvent::KvConnected { client: Arc::from(c), url: clean_url, db_type },
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            } else if db_type == "mongodb" {
                match connectors::connect_nosql(&db_type, &clean_url).await {
                    Ok(c)  => DbEvent::NoSqlConnected { client: Arc::from(c), url: clean_url, db_type },
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            } else {
                match connectors::connect_sql(&db_type, &clean_url).await {
                    Ok(c)  => DbEvent::SqlConnected { client: Arc::from(c), url: clean_url, db_type },
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            };
            let _ = tx.send(event).await;
        });
    }

    fn spawn_post_disconnect(&mut self) {
        if let Some(script) = self.post_disconnect_script.take() {
            tokio::spawn(async move {
                let _ = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&script)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await;
            });
        }
    }

    async fn run_post_disconnect(&mut self) {
        if let Some(script) = self.post_disconnect_script.take() {
            let _ = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&script)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
        }
    }

    // ── Async table listing ───────────────────────────────────────────────────

    fn spawn_load_tables(&mut self) {
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::Sql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.get_table_objects().await {
                        Ok(objs) => DbEvent::TableObjectsLoaded(objs),
                        Err(e)   => DbEvent::TablesLoadFailed(e.to_string()),
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
            Some(ActiveClient::NoSql(c)) => {
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let ev = match c.list_collections().await {
                        Ok(objs) => DbEvent::TableObjectsLoaded(objs),
                        Err(e)   => DbEvent::TablesLoadFailed(e.to_string()),
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
        self.data_grid_screen.prod_readonly = self.prod_readonly;
        self.data_grid_screen.is_nosql = matches!(self.active_client, Some(ActiveClient::NoSql(_)));
        self.fk_history.clear();
        self.state = AppState::DataGrid;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            let empty = BTreeMap::new();
            spawn_sql_page(Arc::clone(c), &table_name, &empty, 0, true, vec![], self.db_tx.clone());
            spawn_sql_count(Arc::clone(c), &table_name, &empty, vec![], self.db_tx.clone());
            let tx = self.db_tx.clone();
            let client = Arc::clone(c);
            tokio::spawn(async move {
                let ev = match client.get_schema(&table_name).await {
                    Ok(s)  => DbEvent::SchemaLoaded(s),
                    Err(_) => DbEvent::SchemaLoadFailed,
                };
                let _ = tx.send(ev).await;
            });
        } else if let Some(ActiveClient::Kv(_)) = &self.active_client {
            self.spawn_load_kv_key(table_name);
        } else if let Some(ActiveClient::NoSql(c)) = &self.active_client {
            let c = Arc::clone(c);
            let tx = self.db_tx.clone();
            self.data_grid_screen.read_only = self.prod_readonly;
            tokio::spawn(async move {
                let count_ev = match c.count(&table_name, "{}").await {
                    Ok(n)  => DbEvent::DataCountLoaded(n),
                    Err(_) => DbEvent::DataCountLoaded(0),
                };
                let page_ev = match c.find(&table_name, "{}", PAGE_SIZE as u64, 0).await {
                    Ok(r)  => DbEvent::DataPageLoaded(r),
                    Err(e) => DbEvent::DataLoadFailed(e.to_string()),
                };
                let _ = tx.send(count_ev).await;
                let _ = tx.send(page_ev).await;
            });
        } else {
            self.data_grid_screen.set_error("Not connected".into());
        }
    }

    fn spawn_load_kv_key(&mut self, key: String) {
        self.data_grid_screen = DataGridScreen::new(key.clone());
        self.data_grid_screen.read_only = true;
        self.data_grid_screen.loading = true;
        self.fk_history.clear();
        self.state = AppState::DataGrid;

        if let Some(ActiveClient::Kv(c)) = &self.active_client {
            let c = Arc::clone(c);
            let tx = self.db_tx.clone();
            tokio::spawn(async move {
                let ttl = c.ttl(&key).await.unwrap_or(-1);
                let ev = match c.get_key_detail(&key).await {
                    Ok(detail) => DbEvent::KvKeyLoaded { detail, key, ttl },
                    Err(e)     => DbEvent::DataLoadFailed(e.to_string()),
                };
                let _ = tx.send(ev).await;
            });
        }
    }

    // Load next page (infinite scroll — triggered when user reaches last row)
    fn spawn_load_more(&mut self) {
        if self.data_grid_screen.loading { return; }
        self.data_grid_screen.loading = true;

        let table   = self.data_grid_screen.table_name.clone();
        let filters = self.data_grid_screen.filters.clone();
        let offset  = self.data_grid_screen.loaded_count;
        let schema  = self.data_grid_screen.schema.clone().unwrap_or_default();

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            spawn_sql_page(Arc::clone(c), &table, &filters, offset, false, schema, self.db_tx.clone());
        } else if let Some(ActiveClient::NoSql(c)) = &self.active_client {
            let c = Arc::clone(c);
            let tx = self.db_tx.clone();
            tokio::spawn(async move {
                let ev = match c.find(&table, "{}", PAGE_SIZE as u64, offset as u64).await {
                    Ok(r)  => DbEvent::DataPageLoaded(r),
                    Err(e) => DbEvent::DataLoadFailed(e.to_string()),
                };
                let _ = tx.send(ev).await;
            });
        }
    }

    // Re-fetch from page 0 with the current filters (after filter add/remove)
    fn spawn_reload_filters(&mut self) {
        let table   = self.data_grid_screen.table_name.clone();
        let filters = self.data_grid_screen.filters.clone();
        let schema  = self.data_grid_screen.schema.clone().unwrap_or_default();
        self.data_grid_screen.reset_data(); // keeps table_name + filters
        // reset_data sets loading=true but doesn't spawn — we do it here
        self.data_grid_screen.total_count = None;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            spawn_sql_page(Arc::clone(c), &table, &filters, 0, true, schema.clone(), self.db_tx.clone());
            spawn_sql_count(Arc::clone(c), &table, &filters, schema, self.db_tx.clone());
        } else {
            self.data_grid_screen.set_error("Not connected to a SQL database".into());
        }
    }

    // ── Async SQL execution (SQL editor) ──────────────────────────────────────

    fn spawn_execute_query(&mut self, sql: String) {
        if self.prod_readonly && !is_select_query(&sql) {
            self.sql_editor_screen.set_error("Read-only mode — write statements are disabled".into());
            return;
        }
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
                        let stmts = split_sql_statements(&sql);
                        if stmts.len() <= 1 {
                            match c.execute(&sql).await {
                                Ok(n)  => DbEvent::QueryExecuted(n),
                                Err(e) => DbEvent::QueryFailed(e.to_string()),
                            }
                        } else {
                            let total_stmts = stmts.len();
                            let mut total = 0u64;
                            let mut failed = None;
                            for (i, stmt) in stmts.into_iter().enumerate() {
                                match c.execute(&stmt).await {
                                    Ok(n)  => total += n,
                                    Err(e) => {
                                        let preview: String = stmt.lines().take(5).collect::<Vec<_>>().join(" | ");
                                        let preview: String = preview.chars().take(120).collect();
                                        failed = Some(format!(
                                            "Statement {}/{} failed: {}\n  → {}…",
                                            i + 1, total_stmts, e, preview
                                        ));
                                        break;
                                    }
                                }
                            }
                            match failed {
                                Some(e) => DbEvent::QueryFailed(e),
                                None    => DbEvent::QueryExecuted(total),
                            }
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
            Some(ActiveClient::NoSql(c)) => {
                let collection = match &self.sql_editor_screen.nosql_collection {
                    Some(c) => c.clone(),
                    None => {
                        self.sql_editor_screen.set_error(
                            "No collection selected — open the editor from the collection list".into(),
                        );
                        return;
                    }
                };
                let c = Arc::clone(c);
                tokio::spawn(async move {
                    let query = sql.trim();
                    let ev = if query.starts_with('[') {
                        match c.aggregate(&collection, query).await {
                            Ok(r)  => DbEvent::QueryRows(r),
                            Err(e) => DbEvent::QueryFailed(e.to_string()),
                        }
                    } else {
                        match c.find(&collection, query, 500, 0).await {
                            Ok(r)  => DbEvent::QueryRows(r),
                            Err(e) => DbEvent::QueryFailed(e.to_string()),
                        }
                    };
                    let _ = tx.send(ev).await;
                });
            }
            None => {
                self.sql_editor_screen.set_error("Not connected".into());
            }
        }
    }

    // ── Export ────────────────────────────────────────────────────────────────

    fn run_export(
        &mut self,
        result: &DbQueryResult,
        table_name: &str,
        schema: Option<Vec<ColumnSchema>>,
        as_json: bool,
    ) {
        if as_json {
            if let (Some(s), Some(ActiveClient::Sql(c))) = (schema, &self.active_client) {
                // Async path: FK resolution
                let client = Arc::clone(c);
                let tx = self.db_tx.clone();
                let result = result.clone();
                let table = table_name.to_string();
                tokio::spawn(async move {
                    let ev = match export::export_json_with_fk(client, &result, &table, &s, 3).await {
                        Ok(p)  => DbEvent::ExportDone(p),
                        Err(e) => DbEvent::ExportFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
                self.status_message = Some(("JSON export with FK resolution in progress…".into(), false));
                self.status_message_ttl = 40;
            } else {
                // Fallback: no SQL client or no schema (e.g. SQL result grid)
                match export::export_json(result, table_name) {
                    Ok(path) => {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        self.status_message = Some((format!("Saved: ~/{name}"), false));
                        self.status_message_ttl = 80;
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Export failed: {e}"), true));
                        self.status_message_ttl = 80;
                    }
                }
            }
        } else {
            match export::export_csv(result, table_name) {
                Ok(path) => {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    self.status_message = Some((format!("Saved: ~/{name}"), false));
                    self.status_message_ttl = 80;
                }
                Err(e) => {
                    self.status_message = Some((format!("Export failed: {e}"), true));
                    self.status_message_ttl = 80;
                }
            }
        }
    }

    // ── DB event handler ──────────────────────────────────────────────────────

    fn handle_db_event(&mut self, event: DbEvent) {
        match event {
            DbEvent::SqlConnected { client, url, db_type } => {
                self.active_client = Some(ActiveClient::Sql(client));
                let safe_url = redact_url(&url);
                self.connected_db_info = Some(format!("[{db_type}] {safe_url}"));
                self.connection_screen.status = None;
                self.table_list_screen = TableListScreen::new();
                self.table_list_screen.db_info = format!("[{db_type}] {safe_url}");
                self.state = AppState::TableList;
                self.spawn_load_tables();
            }
            DbEvent::KvConnected { client, url, db_type } => {
                self.active_client = Some(ActiveClient::Kv(client));
                let safe_url = redact_url(&url);
                self.connected_db_info = Some(format!("[{db_type}] {safe_url}"));
                self.connection_screen.status = None;
                self.table_list_screen = TableListScreen::new();
                self.table_list_screen.db_info = format!("[{db_type}] {safe_url}");
                self.state = AppState::TableList;
                self.spawn_load_tables();
            }
            DbEvent::NoSqlConnected { client, url, db_type } => {
                self.active_client = Some(ActiveClient::NoSql(client));
                let safe_url = redact_url(&url);
                self.connected_db_info = Some(format!("[{db_type}] {safe_url}"));
                self.connection_screen.status = None;
                self.table_list_screen = TableListScreen::new();
                self.table_list_screen.db_info = format!("[{db_type}] {safe_url}");
                self.state = AppState::TableList;
                self.spawn_load_tables();
            }
            DbEvent::ConnectionFailed(msg) => {
                self.connection_screen.status = Some(format!("Error: {msg}"));
            }
            DbEvent::TablesLoaded(names) => {
                self.table_list_screen.set_tables_kv(names);
            }
            DbEvent::TableObjectsLoaded(objects) => {
                self.table_list_screen.set_tables(objects);
                self.spawn_load_all_schemas();
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
            DbEvent::SchemaLoadFailed => {
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
                let schema  = self.data_grid_screen.schema.clone().unwrap_or_default();
                self.data_grid_screen.reset_data();
                self.data_grid_screen.total_count = None;
                if let Some(ActiveClient::Sql(c)) = &self.active_client {
                    spawn_sql_page(Arc::clone(c), &table, &filters, 0, true, schema.clone(), self.db_tx.clone());
                    spawn_sql_count(Arc::clone(c), &table, &filters, schema, self.db_tx.clone());
                } else if let Some(ActiveClient::NoSql(c)) = &self.active_client {
                    let c = Arc::clone(c);
                    let tx = self.db_tx.clone();
                    tokio::spawn(async move {
                        let count_ev = match c.count(&table, "{}").await {
                            Ok(n)  => DbEvent::DataCountLoaded(n),
                            Err(_) => DbEvent::DataCountLoaded(0),
                        };
                        let page_ev = match c.find(&table, "{}", PAGE_SIZE as u64, 0).await {
                            Ok(r)  => DbEvent::DataPageLoaded(r),
                            Err(e) => DbEvent::DataLoadFailed(e.to_string()),
                        };
                        let _ = tx.send(count_ev).await;
                        let _ = tx.send(page_ev).await;
                    });
                }
            }
            DbEvent::EditFailed(msg) => {
                self.modal = Some(Modal::error("Save Failed", &msg));
            }
            DbEvent::KvKeyLoaded { detail, key, ttl } => {
                let ttl_label = match ttl {
                    -1 => "no expiry".to_string(),
                    -2 => "expired".to_string(),
                    n  => format!("TTL: {n}s"),
                };
                self.data_grid_screen.display_name = Some(format!("{key} [{ttl_label}]"));
                let result = kv_detail_to_result(detail);
                let count = result.rows.len() as u64;
                self.data_grid_screen.set_result(result);
                self.data_grid_screen.total_count = Some(count);
                self.data_grid_screen.has_more = false;
            }
            DbEvent::AllSchemasLoaded(schemas) => {
                self.table_list_screen.set_all_schemas(schemas);
            }
            DbEvent::ExportDone(path) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.status_message = Some((format!("Saved: ~/{name}"), false));
                self.status_message_ttl = 80;
            }
            DbEvent::ExportFailed(msg) => {
                self.status_message = Some((format!("Export failed: {msg}"), true));
                self.status_message_ttl = 80;
            }
            DbEvent::StatusUpdate(msg) => {
                self.connection_screen.status = Some(msg);
            }
        }
    }
}

// ── Redis key detail → DbQueryResult ─────────────────────────────────────────

fn kv_detail_to_result(detail: KvKeyDetail) -> DbQueryResult {
    let col = |n: &str| Column { name: n.to_string(), type_name: String::new() };
    let txt = |s: String| Value::Text(s);

    match detail {
        KvKeyDetail::String(v) => DbQueryResult {
            columns: vec![col("value")],
            rows: vec![Row { values: vec![txt(v)] }],
            rows_affected: 0,
        },
        KvKeyDetail::Hash(pairs) => DbQueryResult {
            columns: vec![col("field"), col("value")],
            rows: pairs.into_iter()
                .map(|(f, v)| Row { values: vec![txt(f), txt(v)] })
                .collect(),
            rows_affected: 0,
        },
        KvKeyDetail::List(items) => DbQueryResult {
            columns: vec![col("index"), col("value")],
            rows: items.into_iter().enumerate()
                .map(|(i, v)| Row { values: vec![Value::Int(i as i64), txt(v)] })
                .collect(),
            rows_affected: 0,
        },
        KvKeyDetail::Set(members) => DbQueryResult {
            columns: vec![col("member")],
            rows: members.into_iter()
                .map(|m| Row { values: vec![txt(m)] })
                .collect(),
            rows_affected: 0,
        },
        KvKeyDetail::ZSet(pairs) => DbQueryResult {
            columns: vec![col("member"), col("score")],
            rows: pairs.into_iter()
                .map(|(m, s)| Row { values: vec![txt(m), Value::Float(s)] })
                .collect(),
            rows_affected: 0,
        },
    }
}

// ── Value display helpers ─────────────────────────────────────────────────────

fn json_value_type_and_str(v: &serde_json::Value) -> (String, String) {
    match v {
        serde_json::Value::Bool(b)   => ("bool".into(), b.to_string()),
        serde_json::Value::Number(n) => {
            if n.is_f64() {
                ("float".into(), n.to_string())
            } else {
                ("int".into(), n.to_string())
            }
        }
        serde_json::Value::String(s) => ("string".into(), s.clone()),
        serde_json::Value::Null      => ("string".into(), "NULL".into()),
        serde_json::Value::Object(_) => ("object".into(), v.to_string()),
        serde_json::Value::Array(_)  => ("array".into(), v.to_string()),
    }
}

fn json_object_to_schema_values(json: &str) -> (Vec<ColumnSchema>, Vec<String>) {
    let obj = match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(json) {
        Ok(o) => o,
        Err(_) => return (vec![], vec![]),
    };
    let mut schema = vec![];
    let mut values = vec![];
    for (key, val) in &obj {
        let (type_name, str_val) = json_value_type_and_str(val);
        schema.push(ColumnSchema { name: key.clone(), type_name, is_pk: false, is_nullable: true, fk: None });
        values.push(str_val);
    }
    (schema, values)
}

fn json_array_to_schema_values(json: &str) -> (Vec<ColumnSchema>, Vec<String>) {
    let arr = match serde_json::from_str::<Vec<serde_json::Value>>(json) {
        Ok(a) => a,
        Err(_) => return (vec![], vec![]),
    };
    let mut schema = vec![];
    let mut values = vec![];
    for (i, val) in arr.iter().enumerate() {
        let (type_name, str_val) = json_value_type_and_str(val);
        schema.push(ColumnSchema { name: format!("[{i}]"), type_name, is_pk: false, is_nullable: true, fk: None });
        values.push(str_val);
    }
    (schema, values)
}

fn mongo_type_name(v: &Value) -> String {
    match v {
        Value::Bool(_)        => "bool",
        Value::Int(_)         => "int",
        Value::Float(_)       => "float",
        Value::NestedDoc(_)   => "object",
        Value::NestedArray(_) => "array",
        _                     => "string",
    }.to_string()
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null                              => "NULL".into(),
        Value::Bool(b)                           => b.to_string(),
        Value::Int(i)                            => i.to_string(),
        Value::Float(f)                          => f.to_string(),
        Value::Text(s)                           => s.clone(),
        Value::Bytes(b)                          => format!("<{} bytes>", b.len()),
        Value::NestedDoc(s) | Value::NestedArray(s) => s.clone(),
    }
}

// ── URL redaction ─────────────────────────────────────────────────────────────

fn redact_url(url: &str) -> String {
    let mut result = url.to_string();

    // Mask user:password@ in scheme://user:password@host
    if let Some(at_pos) = result.find('@') {
        if let Some(scheme_end) = result.find("://") {
            let authority_start = scheme_end + 3;
            if authority_start < at_pos {
                let authority = &result[authority_start..at_pos];
                if let Some(colon_pos) = authority.find(':') {
                    let abs_colon = authority_start + colon_pos;
                    result.replace_range(abs_colon + 1..at_pos, "***");
                }
            }
        }
    }

    // Mask sensitive query parameters (authToken, token, password, pwd, secret, key, auth)
    let sensitive = ["authtoken", "token", "password", "pwd", "secret", "key", "auth"];
    if let Some(q_pos) = result.find('?') {
        let base = result[..q_pos + 1].to_string();
        let query = result[q_pos + 1..].to_string();
        let masked: Vec<String> = query.split('&').map(|pair| {
            if let Some(eq) = pair.find('=') {
                let k = pair[..eq].to_ascii_lowercase();
                if sensitive.iter().any(|s| k == *s) {
                    return format!("{}=***", &pair[..eq]);
                }
            }
            pair.to_string()
        }).collect();
        result = format!("{}{}", base, masked.join("&"));
    }

    result
}

// ── Read-only URL param stripping ─────────────────────────────────────────────

fn strip_readonly_param(url: &str) -> (String, bool) {
    let Some(q_pos) = url.find('?') else {
        return (url.to_string(), false);
    };
    let base = &url[..q_pos];
    // Normalize: replace any extra '?' after the first with '&'
    let query = url[q_pos + 1..].replace('?', "&");
    let mut readonly = false;
    let remaining: Vec<&str> = query.split('&').filter(|pair| {
        if let Some(eq) = pair.find('=') {
            if pair[..eq].to_ascii_lowercase() == "readonly"
                && pair[eq + 1..].to_ascii_lowercase() == "true"
            {
                readonly = true;
                return false;
            }
        }
        true
    }).collect();
    let new_url = if remaining.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, remaining.join("&"))
    };
    (new_url, readonly)
}

// ── SQL query helpers ─────────────────────────────────────────────────────────

fn split_sql_statements(sql: &str) -> Vec<String> {
    // Strip -- comment lines before splitting: avoids charset issues with
    // Unicode characters in comments (e.g. box-drawing chars after --).
    // Also strip the comment portion of any inline "sql -- comment" line.
    let cleaned: String = sql
        .lines()
        .map(|line| {
            let t = line.trim_start();
            if t.starts_with("--") {
                return "";
            }
            // Strip inline trailing comment (only when -- is outside a string)
            if let Some(pos) = line.find("--") {
                let before = &line[..pos];
                let in_string = before.chars().filter(|&c| c == '\'').count() % 2 != 0;
                if !in_string { return &line[..pos]; }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");

    cleaned
        .split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

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

fn build_where(filters: &BTreeMap<String, String>, schema: &[ColumnSchema]) -> String {
    if filters.is_empty() { return String::new(); }
    let clauses: Vec<String> = filters.iter()
        .map(|(col, val)| {
            let type_name = schema.iter()
                .find(|cs| cs.name == *col)
                .map(|cs| cs.type_name.as_str())
                .unwrap_or("");
            let tn = type_name.to_uppercase();

            if tn.contains("BOOL") || tn == "TINYINT(1)" {
                let b = matches!(
                    val.to_lowercase().as_str(),
                    "true" | "t" | "1" | "yes" | "on"
                );
                format!("\"{}\" = {}", col, if b { "TRUE" } else { "FALSE" })
            } else if (tn.contains("INT") || tn.contains("FLOAT") || tn.contains("REAL")
                || tn.contains("NUMERIC") || tn.contains("DECIMAL") || tn.contains("DOUBLE")
                || tn.contains("NUMBER"))
                && val.parse::<f64>().is_ok()
            {
                format!("\"{}\" = {}", col, val)
            } else {
                let escaped = val.replace('\'', "''");
                format!("\"{}\" LIKE '%{}%'", col, escaped)
            }
        })
        .collect();
    format!(" WHERE {}", clauses.join(" AND "))
}

fn build_data_query(table: &str, filters: &BTreeMap<String, String>, offset: usize, schema: &[ColumnSchema]) -> String {
    let wh = build_where(filters, schema);
    format!("SELECT * FROM \"{table}\"{wh} LIMIT {PAGE_SIZE} OFFSET {offset}")
}

fn build_count_query(table: &str, filters: &BTreeMap<String, String>, schema: &[ColumnSchema]) -> String {
    let wh = build_where(filters, schema);
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
    schema: Vec<ColumnSchema>,
    tx: mpsc::Sender<DbEvent>,
) {
    let query = build_data_query(table, filters, offset, &schema);
    tokio::spawn(async move {
        let ev = match client.fetch_all(&query).await {
            Ok(r)  => if initial { DbEvent::DataLoaded(r) } else { DbEvent::DataPageLoaded(r) },
            Err(e) => DbEvent::DataLoadFailed(e.to_string()),
        };
        let _ = tx.send(ev).await;
    });
}

// ── Nested document navigation ────────────────────────────────────────────────

/// Returns (col_name, json_string, is_array) if the selected cell holds a nested value.
fn nested_info_from(screen: &DataGridScreen) -> Option<(String, String, bool)> {
    let result = screen.result.as_ref()?;
    let col_idx = screen.selected_col;
    let col_name = result.columns.get(col_idx)?.name.clone();
    let sel_row = screen.table_state.selected()?;
    match result.rows.get(sel_row)?.values.get(col_idx)? {
        Value::NestedDoc(s)   => Some((col_name, s.clone(), false)),
        Value::NestedArray(s) => Some((col_name, s.clone(), true)),
        _ => None,
    }
}

/// Convert a JSON string into a `DbQueryResult`, preserving nested objects/arrays
/// as `Value::NestedDoc` / `Value::NestedArray` for recursive navigation.
fn json_to_result(json: &str, is_array: bool) -> DbQueryResult {
    use crate::db::types::{Column, Row as DbRow};

    let parsed: serde_json::Value = match serde_json::from_str(json) {
        Ok(v)  => v,
        Err(_) => return DbQueryResult {
            columns: vec![Column { name: "value".into(), type_name: "json".into() }],
            rows:    vec![DbRow { values: vec![Value::Text(json.into())] }],
            rows_affected: 0,
        },
    };

    if is_array {
        let items = match parsed.as_array() {
            Some(a) => a.clone(),
            None    => return DbQueryResult { columns: vec![], rows: vec![], rows_affected: 0 },
        };
        if items.is_empty() {
            return DbQueryResult { columns: vec![], rows: vec![], rows_affected: 0 };
        }
        if items[0].is_object() {
            // Array of objects → union of keys as columns, one row per item
            let mut col_names: Vec<String> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for item in &items {
                if let Some(obj) = item.as_object() {
                    for key in obj.keys() {
                        if seen.insert(key.clone()) {
                            col_names.push(key.clone());
                        }
                    }
                }
            }
            let columns: Vec<Column> = col_names.iter()
                .map(|n| Column { name: n.clone(), type_name: "json".into() })
                .collect();
            let rows: Vec<DbRow> = items.iter()
                .map(|item| DbRow {
                    values: col_names.iter()
                        .map(|cn| item.get(cn).map(json_val_to_value).unwrap_or(Value::Null))
                        .collect(),
                })
                .collect();
            DbQueryResult { columns, rows, rows_affected: 0 }
        } else {
            // Scalar array → index + value
            let columns = vec![
                Column { name: "index".into(), type_name: "int".into() },
                Column { name: "value".into(), type_name: "json".into() },
            ];
            let rows: Vec<DbRow> = items.iter().enumerate()
                .map(|(i, v)| DbRow { values: vec![Value::Int(i as i64), json_val_to_value(v)] })
                .collect();
            DbQueryResult { columns, rows, rows_affected: 0 }
        }
    } else {
        // Single object → one row, one column per key
        let obj = match parsed.as_object() {
            Some(o) => o,
            None => return DbQueryResult {
                columns: vec![Column { name: "value".into(), type_name: "json".into() }],
                rows:    vec![DbRow { values: vec![json_val_to_value(&parsed)] }],
                rows_affected: 0,
            },
        };
        let columns: Vec<Column> = obj.keys()
            .map(|k| Column { name: k.clone(), type_name: "json".into() })
            .collect();
        let values: Vec<Value> = obj.values().map(json_val_to_value).collect();
        DbQueryResult { columns, rows: vec![DbRow { values }], rows_affected: 0 }
    }
}

fn json_val_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null       => Value::Null,
        serde_json::Value::Bool(b)    => Value::Bool(*b),
        serde_json::Value::Number(n)  => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else { Value::Float(n.as_f64().unwrap_or(0.0)) }
        }
        serde_json::Value::String(s)  => Value::Text(s.clone()),
        serde_json::Value::Object(_)  => Value::NestedDoc(v.to_string()),
        serde_json::Value::Array(_)   => Value::NestedArray(v.to_string()),
    }
}

fn spawn_sql_count(
    client: Arc<dyn SqlClient>,
    table: &str,
    filters: &BTreeMap<String, String>,
    schema: Vec<ColumnSchema>,
    tx: mpsc::Sender<DbEvent>,
) {
    let query = build_count_query(table, filters, &schema);
    tokio::spawn(async move {
        if let Ok(r) = client.fetch_all(&query).await {
            let _ = tx.send(DbEvent::DataCountLoaded(parse_count(&r))).await;
        }
    });
}
