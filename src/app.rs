use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::{backend::Backend, Terminal};
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{timeout, Duration},
};
use crate::config::Config;
use crate::db::{connectors, traits::{KvClient, SqlClient}, types::DbQueryResult};
use crate::ui::screens::connection::{ConnectionAction, ConnectionScreen};
use crate::ui::screens::data_grid::{DataGridAction, DataGridScreen};
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
    DataLoadFailed(String),
    QueryRows(DbQueryResult),
    QueryExecuted(u64),
    QueryFailed(String),
}

// ── App state machine ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Connection,
    TableList,
    DataGrid,
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
    pub sql_editor_screen: SqlEditorScreen,
    pub active_client: Option<ActiveClient>,
    db_tx: mpsc::Sender<DbEvent>,
    db_rx: mpsc::Receiver<DbEvent>,
}

impl App {
    pub fn new() -> Self {
        let profiles = Config::load().unwrap_or_default().connections;
        let (db_tx, db_rx) = mpsc::channel(8);
        Self {
            state: AppState::Connection,
            should_quit: false,
            connection_screen: ConnectionScreen::new(profiles),
            table_list_screen: TableListScreen::new(),
            data_grid_screen: DataGridScreen::new(String::new()),
            sql_editor_screen: SqlEditorScreen::new(String::new()),
            active_client: None,
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
                    DataGridAction::None => {}
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

    // ── Open SQL editor ───────────────────────────────────────────────────────

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

        match &self.active_client {
            Some(ActiveClient::Sql(c)) => {
                let c = Arc::clone(c);
                let tx = self.db_tx.clone();
                tokio::spawn(async move {
                    let query = format!("SELECT * FROM \"{table_name}\" LIMIT 1000");
                    let ev = match c.fetch_all(&query).await {
                        Ok(r)  => DbEvent::DataLoaded(r),
                        Err(e) => DbEvent::DataLoadFailed(e.to_string()),
                    };
                    let _ = tx.send(ev).await;
                });
            }
            Some(ActiveClient::Kv(_)) => {
                self.data_grid_screen.set_error(
                    "Data Grid not available for key-value stores (Redis). \
                     Use the SQL Editor for queries."
                    .into(),
                );
            }
            None => {
                self.data_grid_screen.set_error("Not connected".into());
            }
        }
    }

    // ── Async SQL execution ───────────────────────────────────────────────────

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
            DbEvent::DataLoadFailed(msg) => {
                self.data_grid_screen.set_error(msg);
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
        }
    }
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
