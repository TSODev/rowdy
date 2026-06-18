use std::collections::BTreeMap;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::{backend::Backend, Terminal};
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{timeout, Duration},
};
use crate::config::{Config, ConnectionProfile, redact_url, strip_readonly_param, resolve_credential};
use crate::export;
use crate::history::QueryHistory;
use crate::db::{
    connectors,
    converters::{json_array_to_schema_values, json_object_to_schema_values, json_to_result,
                 json_val_to_value, json_value_type_and_str, kv_detail_to_result,
                 mongo_type_name, value_to_string},
    query_builder::{build_count_query, build_data_query, build_fk_count_query, build_fk_query,
                    is_select_query, parse_count, split_sql_statements},
    traits::{KvClient, NoSqlClient, SqlClient},
    types::{Column, ColumnSchema, DbQueryResult, KvKeyDetail, Row, TableObject, Value},
};
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

struct ReconnectInfo {
    url: String,
    db_type: String,
}

// ── Async DB events ───────────────────────────────────────────────────────────

pub enum DbEvent {
    SqlConnected   { client: Arc<dyn SqlClient>,   url: String, db_type: String },
    KvConnected    { client: Arc<dyn KvClient>,    url: String, db_type: String },
    NoSqlConnected { client: Arc<dyn NoSqlClient>, url: String, db_type: String },
    ConnectionFailed(String),
    Reconnected(ActiveClient),
    ReconnectFailed(String),
    TablesLoaded(Vec<String>),
    TableObjectsLoaded(Vec<TableObject>),
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

// ── Connection-loss detection ─────────────────────────────────────────────────

fn is_connection_lost(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("connection reset")
        || m.contains("broken pipe")
        || m.contains("connection closed")
        || m.contains("server closed")
        || m.contains("lost connection")
        || m.contains("connection lost")
        || m.contains("transport error")
        || m.contains("network error")
        || m.contains("connection timed out")
        || m.contains("eof")
}

// ── Tab — per-connection state ────────────────────────────────────────────────

pub struct Tab {
    pub state: AppState,
    pub should_quit: bool,
    pub connection_screen: ConnectionScreen,
    pub table_list_screen: TableListScreen,
    pub data_grid_screen: DataGridScreen,
    pub fk_grid_screen: DataGridScreen,
    fk_history: Vec<DataGridScreen>,
    pub sql_result_grid_screen: DataGridScreen,
    pub edit_record_screen: EditRecordScreen,
    edit_record_stack: Vec<(EditRecordScreen, usize)>,
    pub erd_graph_screen: ErdGraphScreen,
    pub sql_editor_screen: SqlEditorScreen,
    pub active_client: Option<ActiveClient>,
    pub connected_db_info: Option<String>,
    pub prod_readonly: bool,
    post_disconnect_script: Option<String>,
    reconnect_info: Option<ReconnectInfo>,
    reconnect_attempt: u8,
    pub reconnecting: bool,
    pub modal: Option<Modal>,
    pending_action: Option<PendingAction>,
    pub status_message: Option<(String, bool)>,
    pub status_message_ttl: u8,
    pub history: QueryHistory,
    edit_origin: AppState,
    db_tx: mpsc::Sender<DbEvent>,
    pub(crate) db_rx: mpsc::Receiver<DbEvent>,
}

impl Tab {
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
            reconnect_info: None,
            reconnect_attempt: 0,
            reconnecting: false,
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

    // ── Key dispatch ──────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        match self.state {
            AppState::Connection => {
                match self.connection_screen.handle_key(key) {
                    ConnectionAction::Quit => self.should_quit = true,
                    ConnectionAction::Connect { url, db_type, pre_connect, post_disconnect, profile_name } => {
                        self.spawn_connect(url, db_type, pre_connect, post_disconnect, profile_name);
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
                                match Config::delete_profile(&profile.name, &profile.url) {
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
                    DataGridAction::ApplyFilter | DataGridAction::ApplySort => self.spawn_reload_filters(),
                    DataGridAction::LoadMore => self.spawn_load_more(),
                    DataGridAction::LoadAll => self.spawn_load_all(),
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
                            self.fk_grid_screen = prev;
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
                    DataGridAction::ApplySort | DataGridAction::LoadAll => {}
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
        child.is_nested = true;
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

    pub fn handle_modal_key(&mut self, key: crossterm::event::KeyEvent) {
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

    fn spawn_connect(&mut self, url: String, db_type: String, pre_connect: Option<String>, post_disconnect: Option<String>, profile_name: Option<String>) {
        let resolved_url = if let Some(ref name) = profile_name {
            match resolve_credential(name, &url) {
                Ok(u) => u,
                Err(e) => {
                    self.connection_screen.status = Some(format!("Keyring error: {e}"));
                    return;
                }
            }
        } else {
            url
        };
        let (clean_url, is_readonly) = strip_readonly_param(&resolved_url);
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

    fn spawn_reconnect(&mut self) {
        let Some(ref info) = self.reconnect_info else { return; };
        let url = info.url.clone();
        let db_type = info.db_type.clone();
        let attempt = self.reconnect_attempt;
        let tx = self.db_tx.clone();

        tokio::spawn(async move {
            let delay = Duration::from_secs(1u64 << attempt.min(2)); // 1s, 2s, 4s
            tokio::time::sleep(delay).await;

            let result = if db_type == "redis" {
                connectors::connect_kv(&db_type, &url).await
                    .map(|c| ActiveClient::Kv(Arc::from(c)))
            } else if db_type == "mongodb" {
                connectors::connect_nosql(&db_type, &url).await
                    .map(|c| ActiveClient::NoSql(Arc::from(c)))
            } else {
                connectors::connect_sql(&db_type, &url).await
                    .map(|c| ActiveClient::Sql(Arc::from(c)))
            };

            let event = match result {
                Ok(client) => DbEvent::Reconnected(client),
                Err(e)     => DbEvent::ReconnectFailed(e.to_string()),
            };
            let _ = tx.send(event).await;
        });
    }

    pub async fn run_post_disconnect(&mut self) {
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
        self.data_grid_screen.sortable = matches!(self.active_client, Some(ActiveClient::Sql(_)));
        self.fk_history.clear();
        self.state = AppState::DataGrid;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            let empty = BTreeMap::new();
            spawn_sql_page(Arc::clone(c), &table_name, &empty, 0, true, vec![], None, None, self.db_tx.clone());
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

    fn spawn_load_more(&mut self) {
        if self.data_grid_screen.loading { return; }
        self.data_grid_screen.loading = true;

        let table    = self.data_grid_screen.table_name.clone();
        let filters  = self.data_grid_screen.filters.clone();
        let offset   = self.data_grid_screen.loaded_count;
        let schema   = self.data_grid_screen.schema.clone().unwrap_or_default();
        let order_by = self.data_grid_screen.sort_col_name.as_ref()
            .map(|n| (n.clone(), self.data_grid_screen.sort_asc));

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            spawn_sql_page(Arc::clone(c), &table, &filters, offset, false, schema, order_by, None, self.db_tx.clone());
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

    fn spawn_reload_filters(&mut self) {
        let table    = self.data_grid_screen.table_name.clone();
        let filters  = self.data_grid_screen.filters.clone();
        let schema   = self.data_grid_screen.schema.clone().unwrap_or_default();
        let order_by = self.data_grid_screen.sort_col_name.as_ref()
            .map(|n| (n.clone(), self.data_grid_screen.sort_asc));
        self.data_grid_screen.reset_data();
        self.data_grid_screen.total_count = None;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            spawn_sql_page(Arc::clone(c), &table, &filters, 0, true, schema.clone(), order_by, None, self.db_tx.clone());
            spawn_sql_count(Arc::clone(c), &table, &filters, schema, self.db_tx.clone());
        } else {
            self.data_grid_screen.set_error("Not connected to a SQL database".into());
        }
    }

    fn spawn_load_all(&mut self) {
        if self.data_grid_screen.loading { return; }
        self.data_grid_screen.loading = true;

        let table    = self.data_grid_screen.table_name.clone();
        let filters  = self.data_grid_screen.filters.clone();
        let schema   = self.data_grid_screen.schema.clone().unwrap_or_default();
        let order_by = self.data_grid_screen.sort_col_name.as_ref()
            .map(|n| (n.clone(), self.data_grid_screen.sort_asc));
        let total    = self.data_grid_screen.total_count.unwrap_or(10_000) as usize;

        if let Some(ActiveClient::Sql(c)) = &self.active_client {
            spawn_sql_page(Arc::clone(c), &table, &filters, 0, true, schema, order_by, Some(total.max(10_000)), self.db_tx.clone());
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

    pub fn handle_db_event(&mut self, event: DbEvent) {
        match event {
            DbEvent::SqlConnected { client, url, db_type } => {
                self.reconnect_info = Some(ReconnectInfo { url: url.clone(), db_type: db_type.clone() });
                self.reconnect_attempt = 0;
                self.reconnecting = false;
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
                self.reconnect_info = Some(ReconnectInfo { url: url.clone(), db_type: db_type.clone() });
                self.reconnect_attempt = 0;
                self.reconnecting = false;
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
                self.reconnect_info = Some(ReconnectInfo { url: url.clone(), db_type: db_type.clone() });
                self.reconnect_attempt = 0;
                self.reconnecting = false;
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
                if !self.reconnecting && is_connection_lost(&msg) {
                    self.reconnecting = true;
                    self.reconnect_attempt = 0;
                    self.spawn_reconnect();
                }
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
                if !self.reconnecting && is_connection_lost(&msg) {
                    self.reconnecting = true;
                    self.reconnect_attempt = 0;
                    self.spawn_reconnect();
                }
                self.data_grid_screen.set_error(msg);
            }
            DbEvent::SchemaLoaded(schema) => {
                self.data_grid_screen.schema = Some(schema);
            }
            DbEvent::SchemaLoadFailed => {}
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
                if !self.reconnecting && is_connection_lost(&msg) {
                    self.reconnecting = true;
                    self.reconnect_attempt = 0;
                    self.spawn_reconnect();
                }
                self.sql_editor_screen.set_error(msg);
            }
            DbEvent::EditSaved => {
                self.state = AppState::DataGrid;
                let table    = self.data_grid_screen.table_name.clone();
                let filters  = self.data_grid_screen.filters.clone();
                let schema   = self.data_grid_screen.schema.clone().unwrap_or_default();
                let order_by = self.data_grid_screen.sort_col_name.as_ref()
                    .map(|n| (n.clone(), self.data_grid_screen.sort_asc));
                self.data_grid_screen.reset_data();
                self.data_grid_screen.total_count = None;
                if let Some(ActiveClient::Sql(c)) = &self.active_client {
                    spawn_sql_page(Arc::clone(c), &table, &filters, 0, true, schema.clone(), order_by, None, self.db_tx.clone());
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
                if !self.reconnecting && is_connection_lost(&msg) {
                    self.reconnecting = true;
                    self.reconnect_attempt = 0;
                    self.spawn_reconnect();
                }
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
                let mut items: Vec<String> = schemas.keys().cloned().collect();
                for cols in schemas.values() {
                    for col in cols {
                        items.push(col.name.clone());
                    }
                }
                items.sort();
                items.dedup();
                self.sql_editor_screen.set_completions(items);
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
            DbEvent::Reconnected(client) => {
                self.active_client = Some(client);
                self.reconnecting = false;
                self.reconnect_attempt = 0;
                self.status_message = Some(("Reconnected".into(), false));
                self.status_message_ttl = 80;
            }
            DbEvent::ReconnectFailed(msg) => {
                if self.reconnect_attempt < 2 {
                    self.reconnect_attempt += 1;
                    self.spawn_reconnect();
                } else {
                    self.reconnecting = false;
                    self.reconnect_attempt = 0;
                    self.status_message = Some((format!("Reconnect failed: {msg}"), true));
                    self.status_message_ttl = 100;
                }
            }
        }
    }

    // ── Tab display name ──────────────────────────────────────────────────────

    pub fn display_name(&self) -> String {
        if let Some(ref info) = self.connected_db_info {
            // Trim to something short: "[postgres] host/db" → "host/db"
            let without_type = info
                .split(']')
                .nth(1)
                .map(|s| s.trim())
                .unwrap_or(info.as_str());
            let short: String = without_type.chars().take(20).collect();
            if short.len() < without_type.len() {
                format!("{short}…")
            } else {
                short
            }
        } else {
            "New connection".to_string()
        }
    }
}

// ── App — multi-tab coordinator ───────────────────────────────────────────────

pub struct App {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            tabs: vec![Tab::new()],
            active_tab: 0,
        }
    }

    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    fn new_tab(&mut self) {
        self.tabs.push(Tab::new());
        self.active_tab = self.tabs.len() - 1;
    }

    fn close_tab(&mut self) {
        if self.tabs.len() == 1 {
            self.tabs[0].should_quit = true;
            return;
        }
        // Run post-disconnect script before closing
        self.tabs[self.active_tab].spawn_post_disconnect();
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    pub async fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut events = EventStream::new();

        loop {
            // Drain events for ALL tabs (background tabs keep their async tasks running)
            for tab in &mut self.tabs {
                while let Ok(ev) = tab.db_rx.try_recv() {
                    tab.handle_db_event(ev);
                }
            }

            terminal.draw(|f| crate::ui::layout::draw(f, self))?;

            if self.tabs[self.active_tab].should_quit {
                self.tabs[self.active_tab].run_post_disconnect().await;
                break;
            }

            if let Ok(Some(Ok(Event::Key(key)))) =
                timeout(Duration::from_millis(50), events.next()).await
            {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.tabs[self.active_tab].should_quit = true;
                } else if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.new_tab();
                } else if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.close_tab();
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    if let KeyCode::Char(c) = key.code {
                        if let Some(n) = c.to_digit(10) {
                            let idx = (n as usize).saturating_sub(1);
                            if idx < self.tabs.len() {
                                self.active_tab = idx;
                            }
                        }
                    }
                } else if self.tabs[self.active_tab].modal.is_some() {
                    self.tabs[self.active_tab].handle_modal_key(key);
                } else {
                    self.tabs[self.active_tab].handle_key(key);
                }
            }
        }

        Ok(())
    }
}

// ── Async spawn helpers ───────────────────────────────────────────────────────

fn spawn_sql_page(
    client: Arc<dyn SqlClient>,
    table: &str,
    filters: &BTreeMap<String, String>,
    offset: usize,
    initial: bool,
    schema: Vec<ColumnSchema>,
    order_by: Option<(String, bool)>,
    limit: Option<usize>,
    tx: mpsc::Sender<DbEvent>,
) {
    let ob_ref = order_by.as_ref().map(|(c, a)| (c.as_str(), *a));
    let query = build_data_query(table, filters, offset, &schema, ob_ref, limit);
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

// ── Nested document navigation ────────────────────────────────────────────────

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
