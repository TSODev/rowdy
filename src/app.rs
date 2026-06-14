use ratatui::{backend::Backend, Terminal};


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
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::Connection,
            should_quit: false,
        }
    }

    pub async fn run<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        while !self.should_quit {
            terminal.draw(|f| crate::ui::layout::draw(f, self))?;
            // TODO: poll crossterm events and dispatch via events::handler
            self.should_quit = true;
        }
        Ok(())
    }
}
