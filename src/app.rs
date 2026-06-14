use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use ratatui::{backend::Backend, Terminal};
use tokio::{
    sync::mpsc,
    time::{timeout, Duration},
};
use crate::config::Config;
use crate::db::{connectors, traits::{KvClient, SqlClient}};
use crate::ui::screens::connection::{ConnectionAction, ConnectionScreen};

// ── Active connection ─────────────────────────────────────────────────────────

pub enum ActiveClient {
    Sql(Box<dyn SqlClient>),
    Kv(Box<dyn KvClient>),
}

// ── Async DB events (sent from spawned tasks back to the main loop) ───────────

pub enum DbEvent {
    SqlConnected(Box<dyn SqlClient>),
    KvConnected(Box<dyn KvClient>),
    ConnectionFailed(String),
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

            // Wait up to 50 ms for a key event; on timeout we loop back so
            // that DB events are picked up promptly without blocking the UI.
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
                    Ok(c)  => DbEvent::KvConnected(c),
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            } else {
                match connectors::connect_sql(&db_type, &url).await {
                    Ok(c)  => DbEvent::SqlConnected(c),
                    Err(e) => DbEvent::ConnectionFailed(e.to_string()),
                }
            };
            let _ = tx.send(event).await;
        });
    }

    // ── DB event handler ──────────────────────────────────────────────────────

    fn handle_db_event(&mut self, event: DbEvent) {
        match event {
            DbEvent::SqlConnected(client) => {
                self.active_client = Some(ActiveClient::Sql(client));
                self.connection_screen.status = None;
                self.state = AppState::TableList;
            }
            DbEvent::KvConnected(client) => {
                self.active_client = Some(ActiveClient::Kv(client));
                self.connection_screen.status = None;
                self.state = AppState::TableList;
            }
            DbEvent::ConnectionFailed(msg) => {
                self.connection_screen.status = Some(format!("Error: {msg}"));
            }
        }
    }
}
