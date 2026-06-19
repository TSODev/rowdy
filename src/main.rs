mod app;
mod config;
mod db;
mod events;
mod export;
mod history;
mod snippets;
mod ui;

use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

#[derive(Parser)]
#[command(
    name    = "rowdy-db",
    version,
    about   = "A TUI database management client for PostgreSQL, SQLite, MySQL and Redis.",
    long_about = None,
)]
struct Cli {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // clap handles --version and --help itself (prints + std::process::exit)
    let _cli = Cli::parse();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(run_tui())
}

async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = app::App::new();
    let result = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
