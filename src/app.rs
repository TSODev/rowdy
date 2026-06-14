use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::{backend::Backend, Terminal};
use std::sync::Arc;
use tokio::{
    sync::mpsc,
    time::{timeout, Duration},
};
use crate::config::Config;
use crate::db::{connectors, traits::{KvClient, SqlClient}};
use crate::ui::screens::connection::{ConnectionAction, ConnectionScreen};
use crate::ui::screens::table_list::{TableListAction, TableListScreen};

// ── Active connection ─────────────────────────────────────────────────────────

// Arc lets us clone a reference into spawned tasks without moving ownership.
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
            // Drain any DB results that arrived since the last frame.
            while let Ok(ev) = self.db_rx.try_recv() {
                self.handle_db_event(ev);
            }

            terminal.draw(|f| crate::ui::layout::draw(f, self))?;

            if self.should_quit {
                break;
            }

            // Wait up to 50 ms for a key; on timeout we loop to check DB events.
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
                    TableListAction::OpenTable(name) => {
                        // TODO: transition to DataGrid for `name`
                        let _ = name;
                        self.state = AppState::DataGrid;
                    }
                    TableListAction::Disconnect => {
                        self.active_client = None;
                        self.table_list_screen = TableListScreen::new();
                        self.state = AppState::Connection;
                    }
                    TableListAction::None => {}
                }
            }
            _ => {
                if key.code == KeyCode::Char('q') {
                    self.should_quit = true;
                }
            }
        }
    }

    // ── Async connection ──────────────────────────────────────────────────────

    fn spawn_connect(&mut self, url: String, db_type: String) {
        self.connection_screen.status =
            Some(format!("Connecting [{db_type}] {url} …"));

        let tx = self.db_tx.clone();
        tokio::spawn(async move {
            let event = if db_type == "redis" {
                match connectors::connect_kv(&db_type, &url).await {
                    Ok(c)  => DbEvent::KvConnected {
                        client: Arc::from(c), url, db_type,
                    },
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            } else {
                match connectors::connect_sql(&db_type, &url).await {
                    Ok(c)  => DbEvent::SqlConnected {
                        client: Arc::from(c), url, db_type,
                    },
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            };
            let _ = tx.send(event).await;
        });
    }

    // ── Async table loading ───────────────────────────────────────────────────

    fn spawn_load_tables(&mut self) {
        let tx = self.db_tx.clone();
        match &self.active_client {
            Some(ActiveClient::Sql(client)) => {
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let event = match client.get_tables().await {
                        Ok(tables) => DbEvent::TablesLoaded(tables),
                        Err(e)     => DbEvent::TablesLoadFailed(e.to_string()),
                    };
                    let _ = tx.send(event).await;
                });
            }
            Some(ActiveClient::Kv(client)) => {
                let client = Arc::clone(client);
                tokio::spawn(async move {
                    let event = match client.keys("*").await {
                        Ok(keys) => DbEvent::TablesLoaded(keys),
                        Err(e)   => DbEvent::TablesLoadFailed(e.to_string()),
                    };
                    let _ = tx.send(event).await;
                });
            }
            None => {}
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
        }
    }
}
