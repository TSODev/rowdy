use crossterm::event::{self, Event, KeyCode};
use ratatui::{backend::Backend, Terminal};
use std::time::Duration;
use crate::config::Config;
use crate::ui::screens::connection::{ConnectionAction, ConnectionScreen};

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Connection,
    TableList,
    DataGrid,
    SqlEditor,
    Quit,
}

pub struct App {
    pub state: AppState,
    pub should_quit: bool,
    pub connection_screen: ConnectionScreen,
}

impl App {
    pub fn new() -> Self {
        let profiles = Config::load().unwrap_or_default().connections;
        Self {
            state: AppState::Connection,
            should_quit: false,
            connection_screen: ConnectionScreen::new(profiles),
        }
    }

    pub async fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        while !self.should_quit {
            terminal.draw(|f| crate::ui::layout::draw(f, self))?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    // Ctrl-C always quits
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(event::KeyModifiers::CONTROL)
                    {
                        self.should_quit = true;
                        continue;
                    }
                    self.handle_key(key);
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        match self.state {
            AppState::Connection => {
                match self.connection_screen.handle_key(key) {
                    ConnectionAction::Quit => self.should_quit = true,
                    ConnectionAction::Connect { url, db_type } => {
                        // TODO: spawn async task to open connection and transition state
                        self.connection_screen.status =
                            Some(format!("Connecting [{db_type}] {url} …"));
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
}
